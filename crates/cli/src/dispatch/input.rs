use concord_core::{Error, Result};
use serde::de::DeserializeOwned;
use std::io::Read;
use std::path::{Path, PathBuf};

const LIMIT: usize = 1024 * 1024;

pub fn read<T: DeserializeOwned>(path: &Path) -> Result<T> {
    let (bytes, label) = if path == Path::new("-") {
        (
            bounded(std::io::stdin().lock(), "stdin")?,
            "stdin".to_string(),
        )
    } else {
        regular(path)?
    };
    serde_json::from_slice(&bytes).map_err(|error| {
        Error::typed(
            "concord.input.json",
            format!("cannot decode JSON change-set from {label}: {error}"),
        )
    })
}

fn regular(source: &Path) -> Result<(Vec<u8>, String)> {
    let metadata = std::fs::symlink_metadata(source).map_err(|error| {
        Error::typed(
            "concord.input.read",
            format!("cannot inspect input {}: {error}", source.display()),
        )
    })?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(Error::typed(
            "concord.input.type",
            format!("input is not a regular file: {}", source.display()),
        ));
    }
    let label = source.display().to_string();
    let file = std::fs::File::open(PathBuf::from(source)).map_err(|error| {
        Error::typed(
            "concord.input.read",
            format!("cannot open input {label}: {error}"),
        )
    })?;
    Ok((bounded(file, &label)?, label))
}

fn bounded(mut reader: impl Read, label: &str) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    reader
        .by_ref()
        .take((LIMIT + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|error| {
            Error::typed(
                "concord.input.read",
                format!("cannot read {label}: {error}"),
            )
        })?;
    if bytes.len() > LIMIT {
        return Err(Error::typed(
            "concord.input.limit",
            format!("input {label} exceeds {LIMIT} bytes"),
        ));
    }
    Ok(bytes)
}
