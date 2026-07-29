use super::{ImportPreflight, MINIMUM_RESERVE, USED_CRIT, used_percent};
use crate::{Error, Result};
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, Default)]
struct ImportSize {
    logical_bytes: u64,
    required_bytes: u64,
    entries: u64,
}

pub(crate) fn preflight(
    source: &Path,
    target: &Path,
    filesystem_path: &Path,
) -> Result<ImportPreflight> {
    if std::fs::symlink_metadata(source)
        .map(|metadata| metadata.file_type().is_symlink())
        .unwrap_or(false)
    {
        return Err(Error::new(format!(
            "resource import refuses symbolic link {}",
            source.display()
        )));
    }
    let source = source.canonicalize().map_err(|error| {
        Error::new(format!(
            "cannot resolve resource source {}: {error}",
            source.display()
        ))
    })?;
    if source.is_dir() && target.starts_with(&source) {
        return Err(Error::new(format!(
            "resource import target {} is inside source {}",
            target.display(),
            source.display()
        )));
    }
    let stats = fs2::statvfs(filesystem_path)?;
    let size = inspect(&source, stats.allocation_granularity())?;
    let reserve_bytes = (stats.total_space() / 4).max(MINIMUM_RESERVE);
    let needed = checked_add(size.required_bytes, reserve_bytes, "resource headroom")?;
    if stats.available_space() < needed {
        return Err(Error::new(format!(
            "resource import requires {} bytes plus {} bytes reserve, but only {} bytes are available",
            size.required_bytes,
            reserve_bytes,
            stats.available_space()
        )));
    }
    let (available_inodes, reserve_inodes) = inode_headroom(filesystem_path, size.entries + 1)?;
    Ok(ImportPreflight {
        source: source.display().to_string(),
        target: target.display().to_string(),
        logical_bytes: size.logical_bytes,
        required_bytes: size.required_bytes,
        entries: size.entries,
        available_bytes: stats.available_space(),
        reserve_bytes,
        available_inodes,
        reserve_inodes,
    })
}

pub(crate) fn ensure_expansion_headroom(path: &Path) -> Result<()> {
    let stats = fs2::statvfs(path)?;
    let used = used_percent(stats.total_space(), stats.available_space());
    if used >= USED_CRIT {
        return Err(Error::new(format!(
            "resource expansion refuses filesystem at {used}% used (critical threshold {USED_CRIT}%)"
        )));
    }
    Ok(())
}

fn inspect(source: &Path, granularity: u64) -> Result<ImportSize> {
    let mut held = ImportSize::default();
    let mut pending = if source.is_dir() {
        child_paths(source)?
    } else {
        vec![source.to_path_buf()]
    };
    while let Some(path) = pending.pop() {
        let metadata = std::fs::symlink_metadata(&path)?;
        let kind = metadata.file_type();
        if kind.is_symlink() {
            return Err(Error::new(format!(
                "resource import refuses symbolic link {}",
                path.display()
            )));
        }
        held.entries = checked_add(held.entries, 1, "resource entry count")?;
        if kind.is_dir() {
            held.required_bytes = checked_add(held.required_bytes, granularity, "resource size")?;
            pending.extend(child_paths(&path)?);
        } else if kind.is_file() {
            held.logical_bytes = checked_add(held.logical_bytes, metadata.len(), "resource size")?;
            held.required_bytes = checked_add(
                held.required_bytes,
                rounded(metadata.len(), granularity)?,
                "resource size",
            )?;
        } else {
            return Err(Error::new(format!(
                "resource import refuses special file {}",
                path.display()
            )));
        }
    }
    held.required_bytes = checked_add(held.required_bytes, granularity, "resource size")?;
    Ok(held)
}

fn child_paths(root: &Path) -> Result<Vec<PathBuf>> {
    std::fs::read_dir(root)?
        .map(|entry| entry.map(|entry| entry.path()).map_err(Error::from))
        .collect()
}

fn rounded(value: u64, granularity: u64) -> Result<u64> {
    if granularity == 0 {
        return Ok(value);
    }
    value
        .checked_add(granularity - 1)
        .map(|value| value / granularity * granularity)
        .ok_or_else(|| Error::new("resource size overflows"))
}

fn checked_add(left: u64, right: u64, label: &str) -> Result<u64> {
    left.checked_add(right)
        .ok_or_else(|| Error::new(format!("{label} overflows")))
}

#[cfg(unix)]
fn inode_headroom(path: &Path, required: u64) -> Result<(Option<u64>, Option<u64>)> {
    let stats = rustix::fs::statvfs(path).map_err(|error| Error::new(error.to_string()))?;
    if stats.f_files == 0 {
        return Ok((None, None));
    }
    let reserve = stats.f_files / 4;
    let needed = checked_add(required, reserve, "resource inode headroom")?;
    if stats.f_favail < needed {
        return Err(Error::new(format!(
            "resource import requires {required} inodes plus {reserve} reserve, but only {} are available",
            stats.f_favail
        )));
    }
    Ok((Some(stats.f_favail), Some(reserve)))
}

#[cfg(not(unix))]
fn inode_headroom(_path: &Path, _required: u64) -> Result<(Option<u64>, Option<u64>)> {
    Ok((None, None))
}
