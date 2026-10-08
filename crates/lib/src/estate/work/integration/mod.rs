mod read;
mod reconcile;
pub(in crate::estate) mod registration;
mod retirement;

use crate::{Error, Result};
use fs2::FileExt;
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::path::PathBuf;

pub(super) const BRANCH: &str = "main";
pub(super) const REMOTE: &str = "origin";
pub(super) const TRACKING: &str = "origin/main";

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Repository {
    pub owner: String,
    pub name: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Integration {
    pub key: i64,
    pub node: String,
    pub repository: Repository,
    pub path: String,
    pub common: String,
    pub remote: String,
    pub branch: String,
}

pub struct Guard {
    pub(in crate::estate) integration: Integration,
    _file: File,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Register {
    pub node: String,
    pub repository: Repository,
    pub path: PathBuf,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Rename {
    pub node: String,
    pub repository: Repository,
}

pub(in crate::estate) fn guard(estate: &crate::estate::Estate, held: &Integration) -> Result<File> {
    let path = estate
        .space
        .join(".concord")
        .join(format!("integration-{}.lock", held.key));
    let file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&path)?;
    crate::path::at(&path).mode(0o600)?;
    file.lock_exclusive()?;
    Ok(file)
}

impl Integration {
    pub fn guard(&self, estate: &crate::estate::Estate) -> Result<Guard> {
        let integration = self.clone();
        let file = guard(estate, &integration)?;
        Ok(Guard {
            integration,
            _file: file,
        })
    }
}

impl Repository {
    pub fn parse(raw: &str) -> Result<Self> {
        let (owner, name) = raw.split_once('/').ok_or_else(repository)?;
        if owner.is_empty() || name.is_empty() {
            return Err(repository());
        }
        if name.contains('/') {
            return Err(repository());
        }
        if [owner, name]
            .iter()
            .any(|part| part.chars().any(char::is_whitespace))
        {
            return Err(repository());
        }
        Ok(Self {
            owner: owner.to_string(),
            name: name.to_string(),
        })
    }

    pub fn identity(&self) -> String {
        format!("{}/{}", self.owner, self.name)
    }

    pub(super) fn validate(&self) -> Result<()> {
        Self::parse(&self.identity()).map(|_| ())
    }
}

pub(in crate::estate) fn decode(row: &keel::Row) -> Result<Integration> {
    let text = |field| {
        row.text(field)
            .map(str::to_string)
            .ok_or_else(|| malformed(row.key(), field))
    };
    let repository = Repository {
        owner: text("owner")?,
        name: text("repository")?,
    };
    repository.validate()?;
    Ok(Integration {
        key: row.key(),
        node: text("node")?,
        repository,
        path: text("path")?,
        common: text("common")?,
        remote: text("remote")?,
        branch: text("branch")?,
    })
}

pub(super) fn node(value: &str) -> Result<()> {
    crate::component("repository node", value)?;
    if value.chars().any(char::is_whitespace) {
        return Err(Error::typed(
            "concord.integration.node",
            "repository node cannot contain whitespace",
        ));
    }
    Ok(())
}

fn malformed(key: i64, field: &str) -> Error {
    Error::typed(
        "concord.integration.row",
        format!("Integration {key} has malformed field {field}"),
    )
}

fn repository() -> Error {
    Error::typed(
        "concord.integration.coordinate",
        "repository must be OWNER/REPOSITORY",
    )
}
