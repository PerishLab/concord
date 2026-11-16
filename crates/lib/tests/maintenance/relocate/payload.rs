use super::{Result, files, member::Member, refuse};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::{Component, Path, PathBuf};

#[derive(Deserialize, Serialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub(super) struct Payload {
    device: u64,
    inode: u64,
    entries: BTreeMap<PathBuf, Entry>,
}

#[derive(Deserialize, Serialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
enum Entry {
    Directory(u32),
    File(String),
    Link(PathBuf),
}

pub(super) fn inventory(root: &Path) -> Result<Payload> {
    files::directory(root)?;
    let metadata = root.metadata()?;
    let mut payload = Payload {
        device: metadata.dev(),
        inode: metadata.ino(),
        entries: BTreeMap::new(),
    };
    walk(root, Path::new(""), &mut payload)?;
    Ok(payload)
}

fn walk(root: &Path, relative: &Path, payload: &mut Payload) -> Result<()> {
    for entry in std::fs::read_dir(root.join(relative))? {
        let entry = entry?;
        let path = relative.join(entry.file_name());
        if payload.entries.len() >= 1_000_000 {
            return Err(refuse("payload observation exhausted"));
        }
        let metadata = entry.path().symlink_metadata()?;
        if metadata.dev() != payload.device {
            return Err(refuse("nested filesystems are unqualified"));
        }
        let value = if metadata.is_symlink() {
            Entry::Link(std::fs::read_link(entry.path())?)
        } else if metadata.is_dir() {
            walk(root, &path, payload)?;
            Entry::Directory(metadata.permissions().mode() & 0o7777)
        } else {
            Entry::File(files::digest(&entry.path())?)
        };
        payload.entries.insert(path, value);
    }
    Ok(())
}

impl Payload {
    pub(super) fn check(&self, root: &Path) -> Result<()> {
        if inventory(root)? != *self {
            return Err(refuse("Issue payload changed; preserve it"));
        }
        Ok(())
    }

    pub(super) fn qualify(&self, nodes: &[String], members: &[Member]) -> Result<()> {
        for (path, entry) in &self.entries {
            if !path
                .components()
                .all(|part| matches!(part, Component::Normal(_)))
            {
                return Err(refuse("unanchored payload path"));
            }
            let parts = path.iter().collect::<Vec<_>>();
            if !nodes
                .iter()
                .any(|node| parts.first().is_some_and(|part| *part == node.as_str()))
            {
                return Err(refuse("orphan Issue payload refuses"));
            }
            if parts.len() <= 2 && !matches!(entry, Entry::Directory(_)) {
                return Err(refuse("Issue seat/container must be a direct directory"));
            }
            let worktree = parts.get(1).is_some_and(|part| *part == "worktree");
            if parts.len() == 3 && !worktree && !matches!(entry, Entry::Directory(_)) {
                return Err(refuse("Artifact seat must be a direct directory"));
            }
            if parts.len() > 1 && !worktree && parts[1] != "artifacts" {
                return Err(refuse("unknown Issue seat entry"));
            }
            if worktree
                && !members
                    .iter()
                    .any(|member| parts[0] == member.node.as_str())
            {
                return Err(refuse("unregistered worktree payload refuses"));
            }
            if worktree && parts.len() > 3 && parts.last().is_some_and(|part| *part == ".git") {
                return Err(refuse(
                    "nested Git repositories require separate qualification",
                ));
            }
            if let Entry::Link(target) = entry {
                if !worktree {
                    return Err(refuse("Artifact/control links are unqualified"));
                }
                link(path, target)?;
            }
        }
        Ok(())
    }

    pub(super) fn links(&self, old: &Path) -> Result<()> {
        for entry in self.entries.values() {
            if let Entry::Link(target) = entry {
                let retired = target.starts_with(old)
                    || target.starts_with(old.parent().unwrap().join(".concord"));
                if target.is_absolute() && retired {
                    return Err(refuse("absolute link targets the old Issue tree"));
                }
            }
        }
        Ok(())
    }
}

fn link(path: &Path, target: &Path) -> Result<()> {
    if target.is_absolute() {
        if target
            .components()
            .any(|part| matches!(part, Component::ParentDir))
        {
            return Err(refuse("absolute link with parent traversal is unqualified"));
        }
        return Ok(());
    }
    let mut normal = false;
    let mut depth = path.parent().unwrap().components().count();
    for part in target.components() {
        match part {
            Component::ParentDir if normal => {
                return Err(refuse(
                    "interior parent traversal in link requires separate qualification",
                ));
            }
            Component::ParentDir if depth == 0 => {
                return Err(refuse("relative link escapes moved Issue tree"));
            }
            Component::ParentDir => depth -= 1,
            Component::Normal(_) => {
                normal = true;
                depth += 1;
            }
            Component::CurDir => (),
            _ => return Err(refuse("unqualified link target")),
        }
    }
    Ok(())
}
