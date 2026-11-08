use super::{Cutover, Intent, Result, Seat, files, journal, refuse, validate};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Ready {
    space: String,
    files: files::Inventory,
}

async fn exact(home: &Path, ready: &Ready) -> Result<()> {
    ready.files.check()?;
    if files::inventory(&home.join("workspace"))? != ready.files
        || validate(home).await? != ready.space
    {
        return Err(refuse("prepared workspace changed; preserve it and stop"));
    }
    Ok(())
}

fn discard(intent: &Intent) -> Result<()> {
    let staged = journal::stage(&intent.home);
    if !staged.try_exists()? {
        return Ok(());
    }
    let inventory = files::inventory(&staged)?;
    for (path, hash) in &inventory.files {
        let allowed = [
            "workspace/estate.sqlite3",
            "workspace/sudo",
            "workspace/identity.json",
            "workspace/locks/estate.lock",
        ]
        .iter()
        .any(|value| path == Path::new(value));
        let artifact = path
            .strip_prefix("workspace/issues")
            .ok()
            .and_then(|path| intent.artifacts.as_ref()?.files.get(path));
        if !allowed && artifact != Some(hash) {
            return Err(refuse("unknown staged payload refuses abort"));
        }
    }
    files::erase(&staged, &inventory)
}

fn cleanup(home: &Path) -> Result<()> {
    let root = journal::root(home);
    let staged = journal::stage(home);
    if staged.try_exists()? {
        fs::remove_dir(staged)?;
    }
    let ready = root.join("ready.json");
    if ready.try_exists()? {
        fs::remove_file(ready)?;
    }
    for name in ["intent.pending", "ready.pending"] {
        let path = root.join(name);
        if path.try_exists()? {
            files::digest(&path)?;
            fs::remove_file(path)?;
        }
    }
    let intent = root.join("intent.json");
    if intent.try_exists()? {
        fs::remove_file(intent)?;
    }
    fs::remove_dir(root)?;
    Ok(())
}

fn supported(controls: &files::Inventory) -> Result<()> {
    for path in controls.files.keys() {
        let text = path.to_string_lossy().replace('\\', "/");
        let observation = text.starts_with("activity/") || text.starts_with("occupancy/");
        let prefix = text.starts_with("integration-") || text.starts_with("land-");
        let extension = text.ends_with(".lock") || text.ends_with(".json");
        let lock = prefix && extension && !text.contains('/');
        let control = text == "estate.sqlite3" || text == "sudo";
        if !control && !observation && !lock {
            return Err(refuse(
                "unknown control files or SQLite sidecars refuse; close/checkpoint through the owning version",
            ));
        }
    }
    for required in ["estate.sqlite3", "sudo"] {
        if !controls.files.contains_key(Path::new(required)) {
            return Err(refuse("missing source database or possession"));
        }
    }
    Ok(())
}

async fn artifacts(intent: &Intent, staged: &Path) -> Result<()> {
    let Some(inventory) = &intent.artifacts else {
        return Ok(());
    };
    let estate = Seat::new(staged).open().await?;
    let nodes = estate
        .issues()
        .await?
        .into_iter()
        .map(|anchor| anchor.node)
        .collect::<Vec<_>>();
    for path in inventory.directories.iter().chain(inventory.files.keys()) {
        let components = path.iter().collect::<Vec<_>>();
        let node = components[0]
            .to_str()
            .ok_or_else(|| refuse("non-UTF8 Artifact path"))?;
        if !nodes.iter().any(|known| known == node) {
            return Err(refuse("orphan Issue payload refuses"));
        }
        if components.len() > 1 && components[1] != "artifacts" {
            return Err(refuse(
                "Member or unknown Issue payload refuses; preserve and drain first",
            ));
        }
    }
    Ok(())
}

fn absent(path: &Path) -> Result<()> {
    match path.symlink_metadata() {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
        Ok(_) => Err(refuse(format!("expected absence: {}", path.display()))),
    }
}

#[cfg(unix)]
fn private(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    Ok(())
}

#[cfg(not(unix))]
fn private(_: &Path) -> Result<()> {
    Ok(())
}

impl Cutover {
    pub(super) async fn stage(&self) -> Result<()> {
        let source = &self.source;
        let home = &self.home;
        absent(&home.join("workspace"))?;
        absent(&journal::root(home))?;
        absent(&journal::frozen(source))?;
        let controls = files::inventory(&source.join(".concord"))?;
        supported(&controls)?;
        let intent = Intent {
            schema: "concord.offline-cutover/v1".into(),
            source: source.into(),
            home: home.into(),
            files: controls,
            artifacts: journal::artifacts(source)?,
        };
        let state = home.join("state");
        if !state.try_exists()? {
            fs::create_dir(&state)?;
        }
        files::directory(&state)?;
        fs::create_dir(journal::root(home))?;
        private(&journal::root(home))?;
        journal::write(&journal::root(home).join("intent.json"), &intent)?;
        fs::rename(source.join(".concord/sudo"), journal::frozen(source))?;
        journal::unchanged(&intent)?;
        let staged = journal::stage(home);
        fs::create_dir(&staged)?;
        drop(Seat::new(&staged).bootstrap().await?);
        fs::copy(
            source.join(".concord/estate.sqlite3"),
            staged.join("workspace/estate.sqlite3"),
        )?;
        fs::copy(journal::frozen(source), staged.join("workspace/sudo"))?;
        if let Some(artifacts) = &intent.artifacts {
            files::copy(
                &source.join(".issues"),
                &staged.join("workspace/issues"),
                artifacts,
            )?;
        }
        let space = validate(&staged).await?;
        artifacts(&intent, &staged).await?;
        journal::unchanged(&intent)?;
        let ready = Ready {
            space,
            files: files::inventory(&staged.join("workspace"))?,
        };
        journal::write(&journal::root(home).join("ready.json"), &ready)?;
        Ok(())
    }
    pub(super) async fn activate(&self) -> Result<()> {
        let source = &self.source;
        let home = &self.home;
        let intent = journal::intent(source, home)?;
        journal::unchanged(&intent)?;
        let ready: Ready = journal::read(&journal::root(home).join("ready.json"))?;
        let workspace = home.join("workspace");
        let staged = journal::stage(home);
        if workspace.try_exists()? {
            if staged.join("workspace").try_exists()? {
                return Err(refuse("both staged and active workspaces exist"));
            }
            return exact(home, &ready).await;
        }
        exact(&staged, &ready).await?;
        journal::unchanged(&intent)?;
        fs::rename(staged.join("workspace"), &workspace)?;
        exact(home, &ready).await
    }
    pub(super) fn abort(&self) -> Result<()> {
        let source = &self.source;
        let home = &self.home;
        absent(&home.join("workspace"))?;
        let record = journal::root(home).join("intent.json");
        if !record.try_exists()? {
            files::digest(&source.join(".concord/sudo"))?;
            absent(&journal::frozen(source))?;
            return cleanup(home);
        }
        let intent = journal::intent(source, home)?;
        let authority = journal::frozen(source);
        if authority.try_exists()? {
            journal::unchanged(&intent)?;
        } else if files::inventory(&source.join(".concord"))? != intent.files {
            return Err(refuse("source changed before freeze"));
        }
        discard(&intent)?;
        if authority.try_exists()? {
            fs::rename(authority, source.join(".concord/sudo"))?;
        }
        cleanup(home)
    }
    pub(super) async fn finish(&self) -> Result<()> {
        let source = &self.source;
        let home = &self.home;
        let intent = journal::intent(source, home)?;
        let ready: Ready = journal::read(&journal::root(home).join("ready.json"))?;
        exact(home, &ready).await?;
        let controls = source.join(".concord");
        if controls.try_exists()? {
            if source.join(".concord/sudo").try_exists()? {
                return Err(refuse("old authority reappeared after activation"));
            }
            let mut expected = intent.files.clone();
            let sudo = expected
                .files
                .remove(Path::new("sudo"))
                .ok_or_else(|| refuse("missing source possession record"))?;
            expected.files.insert("sudo.offline".into(), sudo);
            files::erase(&controls, &expected)?;
        }
        let issues = source.join(".issues");
        if let Some(expected) = &intent.artifacts
            && issues.try_exists()?
        {
            files::erase(&issues, expected)?;
        }
        let lock = source.join(".concord.lock");
        if lock.try_exists()? {
            files::digest(&lock)?;
            fs::remove_file(lock)?;
        }
        let staged = journal::stage(home);
        if staged.try_exists()? {
            fs::remove_dir(staged)?;
        }
        let receipt = journal::root(home).join("complete.json");
        if !receipt.try_exists()? {
            let pending = receipt.with_extension("pending");
            if pending.try_exists()? {
                files::digest(&pending)?;
                fs::remove_file(pending)?;
            }
            journal::write(&receipt, &ready.space)?;
        }
        Ok(())
    }
}
