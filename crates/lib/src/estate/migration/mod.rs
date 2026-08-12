mod activate;
mod load;
mod scan;
mod verify;

use super::{Estate, Seat};
use crate::{Error, Result, Space};
use serde::Serialize;
use std::path::PathBuf;

pub const RELEASE: &str = "v0.10.0";

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Evidence {
    pub path: String,
    pub kind: String,
    pub bytes: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mode: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub digest: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Census {
    pub release: String,
    pub fingerprint: String,
    pub domain: usize,
    pub repository: usize,
    pub task: usize,
    pub member: usize,
    pub claim: usize,
    pub boundary: usize,
    pub dependency: usize,
    pub current: usize,
    pub phase: usize,
    pub artifact: usize,
    pub evidence: Vec<Evidence>,
}

pub struct Staging {
    pub root: PathBuf,
    pub census: Census,
    estate: Estate,
}

pub struct Activation {
    pub root: PathBuf,
    pub census: Census,
    estate: Estate,
}

impl Staging {
    pub fn estate(&self) -> &Estate {
        &self.estate
    }

    pub async fn activate(self, legacy: &Space, fingerprint: &str) -> Result<Activation> {
        activate::run(self, legacy, fingerprint).await
    }
}

impl Activation {
    pub fn estate(&self) -> &Estate {
        &self.estate
    }
}

impl Seat {
    pub fn survey(&self, legacy: &Space) -> Result<Census> {
        self.same(legacy)?;
        agreement(legacy)?;
        scan::Source::read(legacy).map(|source| source.census)
    }

    pub async fn stage(&self, legacy: &Space) -> Result<Staging> {
        self.same(legacy)?;
        agreement(legacy)?;
        if self.database().exists() || self.sudo().exists() {
            return Err(Error::typed(
                "concord.migration.estate",
                "active Concord estate already exists",
            ));
        }
        let _guard = legacy.lock()?;
        agreement(legacy)?;
        let source = scan::Source::read(legacy)?;
        let root = self.root.join("migration").join(RELEASE).join("stage");
        if root.exists() {
            return Err(Error::typed(
                "concord.migration.stage",
                format!("migration stage already exists: {}", root.display()),
            ));
        }
        let staged = self.nested(root.clone());
        let estate = match staged.bootstrap().await {
            Ok(estate) => estate,
            Err(error) => {
                let _ = std::fs::remove_dir_all(&root);
                return Err(error);
            }
        };
        let imported = load::write(&estate, &source).await;
        let checked = match imported {
            Ok(()) => verify::check(&estate, &source).await,
            Err(error) => Err(error),
        };
        if let Err(error) = checked {
            drop(estate);
            let _ = std::fs::remove_dir_all(&root);
            return Err(error);
        }
        Ok(Staging {
            root,
            census: source.census,
            estate,
        })
    }

    pub async fn resume(&self, legacy: &Space, fingerprint: &str) -> Result<Staging> {
        self.same(legacy)?;
        agreement(legacy)?;
        if self.database().exists() || self.sudo().exists() {
            return Err(Error::typed(
                "concord.migration.estate",
                "active Concord estate already exists",
            ));
        }
        let _guard = legacy.lock()?;
        agreement(legacy)?;
        let source = scan::Source::read(legacy)?;
        if source.census.fingerprint != fingerprint {
            return Err(Error::typed(
                "concord.migration.changed",
                "legacy Space differs from the expected staged Census",
            ));
        }
        let root = self.root.join("migration").join(RELEASE).join("stage");
        let estate = self.nested(root.clone()).open().await?;
        verify::check(&estate, &source).await?;
        Ok(Staging {
            root,
            census: source.census,
            estate,
        })
    }

    fn same(&self, legacy: &Space) -> Result<()> {
        let held = self.space.canonicalize()?;
        let found = legacy.path().canonicalize()?;
        if held == found {
            return Ok(());
        }
        Err(Error::typed(
            "concord.migration.space",
            format!(
                "Seat Space {} differs from legacy Space {}",
                held.display(),
                found.display()
            ),
        ))
    }
}

fn agreement(legacy: &Space) -> Result<()> {
    let audit = legacy.audit()?;
    if audit.agrees() {
        return Ok(());
    }
    Err(Error::typed(
        "concord.migration.agreement",
        format!(
            "legacy Space has {} agreement fault(s)",
            audit.faults.iter().filter(|fault| fault.gates()).count()
        ),
    ))
}
