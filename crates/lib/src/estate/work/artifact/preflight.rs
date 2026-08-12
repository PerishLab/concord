use crate::{Error, Result};
use serde::Serialize;
use std::path::{Path, PathBuf};

const RESERVE: u64 = 1024 * 1024 * 1024;

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Survey {
    pub source: String,
    pub target: String,
    #[serde(rename = "logical_bytes")]
    pub logical: u64,
    #[serde(rename = "required_bytes")]
    pub required: u64,
    pub entries: u64,
    #[serde(rename = "available_bytes")]
    pub available: u64,
    #[serde(rename = "reserve_bytes")]
    pub reserve: u64,
    #[serde(rename = "available_inodes", skip_serializing_if = "Option::is_none")]
    pub inodes: Option<u64>,
    #[serde(rename = "reserve_inodes", skip_serializing_if = "Option::is_none")]
    pub retained: Option<u64>,
}

#[derive(Clone, Copy, Debug, Default)]
struct Measure {
    logical: u64,
    required: u64,
    entries: u64,
}

pub(super) fn inspect(source: &Path, target: &Path, filesystem: &Path) -> Result<Survey> {
    if std::fs::symlink_metadata(source)
        .map(|metadata| metadata.file_type().is_symlink())
        .unwrap_or(false)
    {
        return Err(Error::new(format!(
            "Artifact import refuses symbolic link {}",
            source.display()
        )));
    }
    let source = source.canonicalize().map_err(|error| {
        Error::new(format!(
            "cannot resolve Artifact source {}: {error}",
            source.display()
        ))
    })?;
    if source.is_dir() && target.starts_with(&source) {
        return Err(Error::new(format!(
            "Artifact target {} is inside source {}",
            target.display(),
            source.display()
        )));
    }
    let stats = fs2::statvfs(filesystem)?;
    let size = measure(&source, stats.allocation_granularity())?;
    let reserve = (stats.total_space() / 4).max(RESERVE);
    let needed = add(size.required, reserve, "Artifact headroom")?;
    if stats.available_space() < needed {
        return Err(Error::new(format!(
            "Artifact import requires {} bytes plus {} bytes reserve, but only {} bytes are available",
            size.required,
            reserve,
            stats.available_space()
        )));
    }
    let (inodes, retained) = inodes(filesystem, size.entries + 1)?;
    Ok(Survey {
        source: source.display().to_string(),
        target: target.display().to_string(),
        logical: size.logical,
        required: size.required,
        entries: size.entries,
        available: stats.available_space(),
        reserve,
        inodes,
        retained,
    })
}

fn measure(source: &Path, granularity: u64) -> Result<Measure> {
    let mut held = Measure::default();
    let mut pending = if source.is_dir() {
        children(source)?
    } else {
        vec![source.to_path_buf()]
    };
    while let Some(path) = pending.pop() {
        let metadata = std::fs::symlink_metadata(&path)?;
        let kind = metadata.file_type();
        if kind.is_symlink() {
            return Err(Error::new(format!(
                "Artifact import refuses symbolic link {}",
                path.display()
            )));
        }
        held.entries = add(held.entries, 1, "Artifact entry count")?;
        if kind.is_dir() {
            held.required = add(held.required, granularity, "Artifact size")?;
            pending.extend(children(&path)?);
        } else if kind.is_file() {
            held.logical = add(held.logical, metadata.len(), "Artifact size")?;
            held.required = add(
                held.required,
                rounded(metadata.len(), granularity)?,
                "Artifact size",
            )?;
        } else {
            return Err(Error::new(format!(
                "Artifact import refuses special file {}",
                path.display()
            )));
        }
    }
    held.required = add(held.required, granularity, "Artifact size")?;
    Ok(held)
}

fn children(root: &Path) -> Result<Vec<PathBuf>> {
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
        .ok_or_else(|| Error::new("Artifact size overflows"))
}

fn add(left: u64, right: u64, label: &str) -> Result<u64> {
    left.checked_add(right)
        .ok_or_else(|| Error::new(format!("{label} overflows")))
}

#[cfg(unix)]
fn inodes(path: &Path, required: u64) -> Result<(Option<u64>, Option<u64>)> {
    let stats = rustix::fs::statvfs(path).map_err(|error| Error::new(error.to_string()))?;
    if stats.f_files == 0 {
        return Ok((None, None));
    }
    let reserve = stats.f_files / 4;
    let needed = add(required, reserve, "Artifact inode headroom")?;
    if stats.f_favail < needed {
        return Err(Error::new(format!(
            "Artifact import requires {required} inodes plus {reserve} reserve, but only {} are available",
            stats.f_favail
        )));
    }
    Ok((Some(stats.f_favail), Some(reserve)))
}

#[cfg(not(unix))]
fn inodes(_: &Path, _: u64) -> Result<(Option<u64>, Option<u64>)> {
    Ok((None, None))
}
