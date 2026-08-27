use crate::{Error, Result};
use sha2::{Digest, Sha256};

pub fn normalize(write: &[String]) -> Result<Vec<String>> {
    if write.is_empty() {
        return Err(Error::new("at least one write claim is required"));
    }
    let mut held = write
        .iter()
        .map(|path| parse(path))
        .collect::<Result<Vec<_>>>()?;
    held.sort();
    held.dedup();
    let mut reduced: Vec<String> = Vec::new();
    for path in held {
        if !reduced.iter().any(|parent| covers(parent, &path)) {
            reduced.push(path);
        }
    }
    Ok(reduced)
}

pub fn digest(write: &[String]) -> String {
    let mut digest = Sha256::new();
    for path in write {
        digest.update(path.as_bytes());
        digest.update([0]);
    }
    format!("{:x}", digest.finalize())
}

pub fn intersections(left: &[String], right: &[String]) -> Vec<String> {
    let mut paths = left
        .iter()
        .flat_map(|left| {
            right.iter().filter_map(move |right| {
                if covers(left, right) {
                    Some(right.clone())
                } else if covers(right, left) {
                    Some(left.clone())
                } else {
                    None
                }
            })
        })
        .collect::<Vec<_>>();
    paths.sort();
    paths.dedup();
    paths
}

pub fn covers(boundary: &str, path: &str) -> bool {
    boundary == "."
        || boundary == path
        || path
            .strip_prefix(boundary)
            .is_some_and(|tail| tail.starts_with('/'))
}

fn parse(path: &str) -> Result<String> {
    if path == "." {
        return Ok(path.to_string());
    }
    if path.is_empty() || path.starts_with('/') {
        return Err(invalid(path));
    }
    if path.contains('\\') || path.contains('\0') {
        return Err(invalid(path));
    }
    for component in path.split('/') {
        if component.is_empty() || matches!(component, "." | "..") {
            return Err(invalid(path));
        }
    }
    Ok(path.to_string())
}

fn invalid(path: &str) -> Error {
    Error::new(format!(
        "write claim is not a canonical repo-relative path: {path}"
    ))
}
