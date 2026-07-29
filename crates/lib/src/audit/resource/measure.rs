use super::{
    Filesystem, HostMemory, Inodes, MEMORY_CRIT, MEMORY_WARN, Observation, Status, used_percent,
    used_status, worst,
};
use crate::{Error, Result};
use std::collections::BTreeSet;
use std::fs::Metadata;
use std::path::{Path, PathBuf};
use sysinfo::{MemoryRefreshKind, RefreshKind, System};

#[derive(Clone, Copy, Debug, Default)]
pub(super) struct Scan {
    pub bytes: u64,
    pub entries: u64,
}

pub(super) fn filesystem(path: &Path) -> Observation<Filesystem> {
    match fs2::statvfs(path) {
        Ok(stats) => {
            let used = used_percent(stats.total_space(), stats.available_space());
            let (inodes, inode_note) = observed_inodes(path);
            let status = inodes.as_ref().map_or_else(
                || used_status(used),
                |held| worst(used_status(used), held.status),
            );
            Observation {
                status,
                value: Some(Filesystem {
                    status,
                    total_bytes: stats.total_space(),
                    available_bytes: stats.available_space(),
                    used_percent: used,
                    inodes,
                    inode_note,
                }),
                note: None,
            }
        }
        Err(error) => Observation {
            status: Status::Unknown,
            value: None,
            note: Some(error.to_string()),
        },
    }
}

fn observed_inodes(path: &Path) -> (Option<Inodes>, Option<String>) {
    match inode_stats(path) {
        Ok(Some(inodes)) => (Some(inodes), None),
        Ok(None) => (
            None,
            Some("inode capacity is unavailable on this platform".into()),
        ),
        Err(error) => (None, Some(error.to_string())),
    }
}

pub(super) fn host_memory() -> Observation<HostMemory> {
    let system =
        System::new_with_specifics(RefreshKind::new().with_memory(MemoryRefreshKind::everything()));
    let total = system.total_memory();
    if total == 0 {
        return Observation {
            status: Status::Unknown,
            value: None,
            note: Some("host memory capacity is unavailable".into()),
        };
    }
    let available = system.available_memory();
    let percent = available.saturating_mul(100) / total;
    let status = if percent <= MEMORY_CRIT {
        Status::Crit
    } else if percent <= MEMORY_WARN {
        Status::Warn
    } else {
        Status::Ok
    };
    Observation {
        status,
        value: Some(HostMemory {
            status,
            total_bytes: total,
            available_bytes: available,
            available_percent: percent,
            swap_total_bytes: system.total_swap(),
            swap_used_bytes: system.used_swap(),
        }),
        note: None,
    }
}

pub(super) fn scan(root: &Path) -> Result<Scan> {
    let mut held = Scan::default();
    let mut pending = vec![root.to_path_buf()];
    let mut links = BTreeSet::new();
    while let Some(path) = pending.pop() {
        let metadata = std::fs::symlink_metadata(&path)?;
        held.entries = held
            .entries
            .checked_add(1)
            .ok_or_else(|| Error::new("resource entry count overflows"))?;
        if count_allocation(&metadata, &mut links) {
            held.bytes = held
                .bytes
                .checked_add(allocated(&metadata))
                .ok_or_else(|| Error::new("resource byte count overflows"))?;
        }
        if metadata.file_type().is_dir() {
            pending.extend(child_paths(&path)?);
        }
    }
    Ok(held)
}

pub(super) fn child_directories(root: &Path) -> Result<Vec<PathBuf>> {
    if !root.exists() {
        return Ok(Vec::new());
    }
    let mut paths = Vec::new();
    for entry in std::fs::read_dir(root)? {
        let entry = entry?;
        if entry.file_type()?.is_dir() {
            paths.push(entry.path());
        }
    }
    paths.sort();
    Ok(paths)
}

fn child_paths(root: &Path) -> Result<Vec<PathBuf>> {
    std::fs::read_dir(root)?
        .map(|entry| entry.map(|entry| entry.path()).map_err(Error::from))
        .collect()
}

#[cfg(unix)]
fn allocated(metadata: &Metadata) -> u64 {
    use std::os::unix::fs::MetadataExt;
    metadata.blocks().saturating_mul(512)
}

#[cfg(not(unix))]
fn allocated(metadata: &Metadata) -> u64 {
    metadata.len()
}

#[cfg(unix)]
fn count_allocation(metadata: &Metadata, links: &mut BTreeSet<(u64, u64)>) -> bool {
    use std::os::unix::fs::MetadataExt;
    !metadata.file_type().is_file()
        || metadata.nlink() <= 1
        || links.insert((metadata.dev(), metadata.ino()))
}

#[cfg(not(unix))]
fn count_allocation(_metadata: &Metadata, _links: &mut BTreeSet<(u64, u64)>) -> bool {
    true
}

#[cfg(unix)]
fn inode_stats(path: &Path) -> Result<Option<Inodes>> {
    let stats = rustix::fs::statvfs(path).map_err(|error| Error::new(error.to_string()))?;
    if stats.f_files == 0 {
        return Ok(None);
    }
    let used = used_percent(stats.f_files, stats.f_favail);
    Ok(Some(Inodes {
        status: used_status(used),
        total: stats.f_files,
        available: stats.f_favail,
        used_percent: used,
    }))
}

#[cfg(not(unix))]
fn inode_stats(_path: &Path) -> Result<Option<Inodes>> {
    Ok(None)
}
