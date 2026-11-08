use super::{Result, refuse};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Inventory {
    pub files: BTreeMap<PathBuf, String>,
    pub directories: Vec<PathBuf>,
}

impl Inventory {
    pub(super) fn check(&self) -> Result<()> {
        for path in self.files.keys().chain(self.directories.iter()) {
            if path.as_os_str().is_empty()
                || !path
                    .components()
                    .all(|part| matches!(part, std::path::Component::Normal(_)))
            {
                return Err(refuse("unanchored inventory path refuses"));
            }
        }
        Ok(())
    }
}

pub(super) fn directory(path: &Path) -> Result<()> {
    let metadata = direct(path)?;
    if !metadata.is_dir() {
        return Err(refuse("expected a direct directory"));
    }
    Ok(())
}

fn direct(path: &Path) -> Result<fs::Metadata> {
    let metadata = path.symlink_metadata()?;
    if metadata.is_symlink() || reparse(&metadata) {
        return Err(refuse(format!(
            "links/reparse points refuse: {}",
            path.display()
        )));
    }
    Ok(metadata)
}

#[cfg(windows)]
fn reparse(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    metadata.file_attributes() & 0x400 != 0
}

#[cfg(not(windows))]
fn reparse(_: &fs::Metadata) -> bool {
    false
}

pub(super) fn digest(path: &Path) -> Result<String> {
    if !direct(path)?.is_file() {
        return Err(refuse("special file refuses"));
    }
    let mut file = File::open(path)?;
    let mut digest = Sha256::new();
    let mut buffer = [0; 65536];
    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        digest.update(&buffer[..count]);
    }
    Ok(format!(
        "{}:{:x}",
        mode(&file.metadata()?),
        digest.finalize()
    ))
}

pub(super) fn inventory(root: &Path) -> Result<Inventory> {
    directory(root)?;
    let mut result = Inventory {
        files: BTreeMap::new(),
        directories: Vec::new(),
    };
    walk(root, Path::new(""), &mut result)?;
    result.directories.sort();
    Ok(result)
}

fn walk(root: &Path, relative: &Path, result: &mut Inventory) -> Result<()> {
    for entry in fs::read_dir(root.join(relative))? {
        let entry = entry?;
        let path = relative.join(entry.file_name());
        let metadata = direct(&entry.path())?;
        if metadata.is_dir() {
            result.directories.push(path.clone());
            walk(root, &path, result)?;
        } else {
            result.files.insert(path, digest(&entry.path())?);
        }
    }
    Ok(())
}

pub(super) fn copy(source: &Path, target: &Path, inventory: &Inventory) -> Result<()> {
    inventory.check()?;
    fs::create_dir(target)?;
    for path in &inventory.directories {
        fs::create_dir_all(target.join(path))?;
    }
    for (path, expected) in &inventory.files {
        let source = source.join(path);
        if digest(&source)? != *expected {
            return Err(refuse("source changed during copy"));
        }
        fs::copy(source, target.join(path))?;
    }
    if self::inventory(target)? != *inventory {
        return Err(refuse("copy hash verification failed"));
    }
    Ok(())
}

pub(super) fn erase(root: &Path, expected: &Inventory) -> Result<()> {
    expected.check()?;
    for (path, hash) in &expected.files {
        let path = root.join(path);
        if !path.try_exists()? {
            continue;
        }
        if digest(&path)? != *hash {
            return Err(refuse("cleanup refuses changed payload"));
        }
        fs::remove_file(path)?;
    }
    let mut directories = expected.directories.clone();
    directories.sort_by_key(|path| std::cmp::Reverse(path.components().count()));
    for path in directories {
        let path = root.join(path);
        if path.try_exists()? {
            fs::remove_dir(path)?;
        }
    }
    fs::remove_dir(root)?;
    Ok(())
}

#[cfg(unix)]
fn mode(metadata: &fs::Metadata) -> u32 {
    use std::os::unix::fs::PermissionsExt;
    metadata.permissions().mode() & 0o7777
}

#[cfg(not(unix))]
fn mode(metadata: &fs::Metadata) -> u32 {
    u32::from(metadata.permissions().readonly())
}
