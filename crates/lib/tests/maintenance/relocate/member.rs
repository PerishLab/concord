use super::{Result, files, git, refuse};
use concord_core::{Estate, Integration};
use serde::{Deserialize, Serialize};
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Member {
    pub node: String,
    pub integration: Integration,
    branch: String,
    head: String,
    directory: PathBuf,
    device: u64,
    inode: u64,
    index: Option<String>,
    pointer: String,
}

pub(super) async fn capture(estate: &Estate, source: &Path) -> Result<Vec<Member>> {
    let mut members = Vec::new();
    for held in estate.issue_worktrees().await? {
        let path = source.join(".issues").join(&held.node).join("worktree");
        files::directory(&path)?;
        let directory = PathBuf::from(
            git::Git::new(&path)?
                .text(&["rev-parse", "--absolute-git-dir"])
                .await?,
        )
        .canonicalize()?;
        let common = PathBuf::from(
            git::Git::new(&path)?
                .text(&["rev-parse", "--path-format=absolute", "--git-common-dir"])
                .await?,
        )
        .canonicalize()?;
        if common != Path::new(&held.integration.common)
            || directory.parent() != Some(common.join("worktrees").as_path())
        {
            return Err(refuse("Member Git custody disagrees"));
        }
        let pointer = format!("gitdir: {}\n", directory.display());
        files::digest(&path.join(".git"))?;
        if std::fs::read_to_string(path.join(".git"))? != pointer {
            return Err(refuse(
                "only canonical absolute Member Git pointers are qualified",
            ));
        }
        if git::Git::new(&path)?
            .optional(&["config", "--get", "core.worktree"])
            .await?
            .is_some()
        {
            return Err(refuse(
                "explicit core.worktree is outside relocation qualification",
            ));
        }
        let metadata = directory.symlink_metadata()?;
        files::directory(&directory)?;
        let member = Member {
            node: held.node,
            integration: held.integration,
            branch: held.branch,
            head: git::Git::new(&path)?
                .text(&["rev-parse", "HEAD^{commit}"])
                .await?,
            device: metadata.dev(),
            inode: metadata.ino(),
            index: index(&directory)?,
            directory,
            pointer,
        };
        member.verify(source, &path, false).await?;
        members.push(member);
    }
    Ok(members)
}

fn index(directory: &Path) -> Result<Option<String>> {
    let path = directory.join("index");
    if path.try_exists()? {
        Ok(Some(files::digest(&path)?))
    } else {
        Ok(None)
    }
}

impl Member {
    pub(super) fn path(&self, issues: &Path) -> PathBuf {
        issues.join(&self.node).join("worktree")
    }

    pub(super) async fn verify(&self, source: &Path, path: &Path, moved: bool) -> Result<()> {
        let metadata = self.directory.symlink_metadata()?;
        files::directory(&self.directory)?;
        if metadata.dev() != self.device
            || metadata.ino() != self.inode
            || index(&self.directory)? != self.index
        {
            return Err(refuse(
                "Member admin/index changed; preserve migration state",
            ));
        }
        for name in ["index.lock", "HEAD.lock", "gitdir.lock", "locked"] {
            if self.directory.join(name).symlink_metadata().is_ok() {
                return Err(refuse(
                    "Member admin is locked; stop writers and preserve state",
                ));
            }
        }
        files::digest(&path.join(".git"))?;
        if std::fs::read_to_string(path.join(".git"))? != self.pointer {
            return Err(refuse("Member Git pointer changed"));
        }
        if git::Git::new(path)?
            .optional(&["config", "--get", "core.worktree"])
            .await?
            .is_some()
        {
            return Err(refuse("explicit core.worktree refuses relocation"));
        }
        let backlink = self.directory.join("gitdir");
        files::digest(&backlink)?;
        let old = self.path(&source.join(".issues")).join(".git");
        let target = path.join(".git");
        let text = std::fs::read_to_string(&backlink)?;
        let actual = Path::new(text.trim());
        if actual != old && (!moved || actual != target) {
            return Err(refuse(
                "Member backlink changed outside owned old/new paths",
            ));
        }
        if git::Git::new(path)?
            .text(&["symbolic-ref", "--short", "HEAD"])
            .await?
            != self.branch
            || git::Git::new(path)?
                .text(&["rev-parse", "HEAD^{commit}"])
                .await?
                != self.head
            || PathBuf::from(
                git::Git::new(path)?
                    .text(&["rev-parse", "--path-format=absolute", "--git-common-dir"])
                    .await?,
            )
            .canonicalize()?
                != Path::new(&self.integration.common)
        {
            return Err(refuse("Member branch/head/repository changed"));
        }
        if moved && actual != target {
            return Err(refuse("Member backlink still requires repair"));
        }
        Ok(())
    }

    pub(super) async fn repair(&self, source: &Path, issues: &Path) -> Result<()> {
        let path = self.path(issues);
        let backlink = self.directory.join("gitdir");
        files::digest(&backlink)?;
        let text = std::fs::read_to_string(&backlink)?;
        let actual = Path::new(text.trim());
        let old = self.path(&source.join(".issues")).join(".git");
        let target = path.join(".git");
        if actual != old && actual != target {
            return Err(refuse("foreign Git backlink refuses repair"));
        }
        self.verify(source, &path, actual == target).await?;
        if actual == old {
            git::Git::new(Path::new(&self.integration.path))?
                .repair(&path)
                .await?;
        }
        self.verify(source, &path, true).await
    }
}

pub(super) async fn repository(held: &Integration) -> Result<()> {
    let root = Path::new(&held.path);
    if PathBuf::from(
        git::Git::new(root)?
            .text(&["rev-parse", "--path-format=absolute", "--git-common-dir"])
            .await?,
    )
    .canonicalize()?
        != Path::new(&held.common)
        || git::Git::new(root)?
            .text(&["symbolic-ref", "--short", "HEAD"])
            .await?
            != held.branch
    {
        return Err(refuse("Integration identity/branch changed"));
    }
    let url = git::Git::new(root)?
        .text(&["config", "--get", "remote.origin.url"])
        .await?;
    let coordinate = url
        .strip_prefix("https://github.com/")
        .or_else(|| url.strip_prefix("http://github.com/"))
        .or_else(|| url.strip_prefix("git@github.com:"))
        .or_else(|| url.strip_prefix("ssh://git@github.com/"))
        .map(|text| text.strip_suffix(".git").unwrap_or(text));
    if coordinate != Some(held.repository.identity().as_str()) {
        return Err(refuse("Integration origin changed"));
    }
    Ok(())
}
