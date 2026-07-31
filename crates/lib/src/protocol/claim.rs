use crate::{Domain, Error, Result, Space, git};
use sha2::{Digest, Sha256};
use std::path::Path;

struct Held {
    owner: String,
    identity: std::path::PathBuf,
    write: Vec<String>,
}

pub struct Wanted<'a> {
    pub task: &'a str,
    pub member: &'a str,
    pub source: &'a Path,
    pub write: &'a [String],
}

pub fn available(space: &Space, wanted: Wanted<'_>) -> Result<()> {
    let identity = git::at(wanted.source).identity()?;
    let owner = format!("{}/{}", wanted.task, wanted.member);
    for held in active(space)? {
        if held.owner == owner || held.identity != identity {
            continue;
        }
        if overlaps(wanted.write, &held.write) {
            return Err(Error::new(format!(
                "write claim overlaps active member {}",
                held.owner
            )));
        }
    }
    Ok(())
}

fn active(space: &Space) -> Result<Vec<Held>> {
    let mut held = Vec::new();
    for domain in space.domains()? {
        held.extend(members(&domain)?);
    }
    Ok(held)
}

fn members(domain: &Domain) -> Result<Vec<Held>> {
    let registry = domain.registry()?;
    let mut held = Vec::new();
    for task in &registry.task {
        let resolved = domain.task(&task.name)?;
        for member in &task.repo {
            let source = resolved.source(&member.source)?;
            let write = if registry.version == 1 {
                vec![".".to_string()]
            } else {
                member.write.clone()
            };
            held.push(Held {
                owner: format!("{}/{}/{}", domain.name(), task.name, member.name),
                identity: git::at(&source).identity()?,
                write,
            });
        }
    }
    Ok(held)
}

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

pub fn overlaps(left: &[String], right: &[String]) -> bool {
    left.iter().any(|left| {
        right
            .iter()
            .any(|right| covers(left, right) || covers(right, left))
    })
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
