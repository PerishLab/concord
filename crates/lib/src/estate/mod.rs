use crate::path::at;
use crate::{Error, Result};
use keel::adapt::db::Sqlite;
use keel::{Core, Status};
use radicula_lock::Mode;
use std::fs::File;
use std::path::{Path, PathBuf};

#[path = "data/agreement.rs"]
mod agreement;
mod forge;
mod layout;
mod model;
#[path = "data/territory/mod.rs"]
mod territory;
mod work;

pub use agreement::{Agreement, Finding};
pub use forge::{Admission, Anchor, Coordinate, Reconcile};
pub use forge::{Reference, ReferenceKind};
pub use work::{
    BoundaryState, CheckoutState, ClaimOverlap, CommittedEvidence, CommittedOverlap, Finish, Guard,
    Integration, IntegrationState, IssueArtifact, IssueClaiming, IssueDeclaration, IssueImport,
    IssueMemberChange, IssueMemberStatus, IssueNarrowing, IssueProving, IssueRelease, IssueRemoval,
    IssueRetirement, IssueWithdrawal, IssueWorktree, Proof, Recovery, Register, Rename, Repository,
    Start, UpstreamState, authority, issue_delivery, landing,
};

pub struct Estate {
    core: Core<Sqlite>,
    layout: layout::Layout,
    identity: String,
}

#[derive(Clone, Debug)]
pub struct Seat {
    layout: layout::Layout,
}

impl Seat {
    pub fn new(home: impl AsRef<Path>) -> Self {
        Self {
            layout: layout::Layout::new(home.as_ref()),
        }
    }

    pub fn database(&self) -> PathBuf {
        self.layout.database()
    }

    pub fn sudo(&self) -> PathBuf {
        self.layout.sudo()
    }

    pub async fn bootstrap(&self) -> Result<Estate> {
        if !self.layout.root.is_absolute() {
            return Err(Error::new("Concord data home must be absolute"));
        }
        if let Some(parent) = self.layout.root.parent() {
            at(parent).directory()?;
        }
        match std::fs::create_dir(&self.layout.root) {
            Ok(()) => (),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                return self.open().await;
            }
            Err(error) => return Err(error.into()),
        }
        at(&self.layout.root).mode(0o700)?;
        at(&self.layout.locks()).directory()?;
        let wire = Sqlite::file(self.database()).await.map_err(fault)?;
        let mut held = keel::bootstrap(model::graph(), wire).map_err(fault)?;
        let status = held.status().await.map_err(fault)?;
        let sudo = self.possession(&mut held, status).await?;
        let core = held.seal(&sudo).await.map_err(fault)?;
        at(&self.database()).mode(0o600)?;
        let identity = self.layout.publish()?;
        let estate = Estate {
            core,
            layout: self.layout.resolved()?,
            identity,
        };
        Ok(estate)
    }

    pub async fn open(&self) -> Result<Estate> {
        if !self.layout.root.exists() {
            return Err(Error::typed(
                "concord.estate.absent",
                format!(
                    "Concord estate is absent at {}; bootstrap a fresh Issue estate",
                    self.layout.root.display()
                ),
            ));
        }
        let identity = self.layout.read()?;
        let sudo = std::fs::read_to_string(self.sudo())?;
        let core = self.bind(model::graph, &sudo).await?;
        Ok(Estate {
            core,
            layout: self.layout.resolved()?,
            identity,
        })
    }

    async fn possession(
        &self,
        held: &mut keel::Bootstrap<Sqlite>,
        status: Status,
    ) -> Result<String> {
        if self.sudo().is_file() {
            return std::fs::read_to_string(self.sudo()).map_err(Into::into);
        }
        if status == Status::Occupied {
            return Err(Error::typed(
                "concord.estate.sudo_absent",
                "occupied Concord estate has no retained sudo possession",
            ));
        }
        let sudo = held.mint().await.map_err(fault)?;
        at(&self.sudo()).file(&sudo)?;
        Ok(sudo)
    }

    async fn bind(&self, model: fn() -> keel::Graph, sudo: &str) -> Result<Core<Sqlite>> {
        let wire = Sqlite::file(self.database()).await.map_err(fault)?;
        let held = keel::bootstrap(model(), wire).map_err(fault)?;
        held.seal(sudo).await.map_err(upgrade)
    }
}

impl Estate {
    pub fn identity(&self) -> &str {
        &self.identity
    }

    pub(crate) fn identify(&self, expected: &str) -> Result<()> {
        if expected != self.identity || self.layout.read()? != self.identity {
            return Err(Error::typed(
                "concord.space.identity",
                "Space identity changed or belongs to another data home; prepare a fresh plan",
            ));
        }
        Ok(())
    }
    pub fn occupy(
        &self,
        operator: &crate::activity::Operator,
        operation: &str,
        subjects: &[crate::occupancy::Subject],
    ) -> Result<crate::occupancy::Occupancy> {
        self.identify(&self.identity)?;
        crate::occupancy::record(&self.layout.root, operator, operation, subjects)
    }

    pub fn write_surfaces(
        &self,
        source: &Path,
        claims: &[String],
    ) -> Result<Vec<crate::occupancy::Subject>> {
        let source = source.canonicalize().map_err(|error| {
            Error::typed(
                "concord.member.source",
                format!("cannot resolve source {}: {error}", source.display()),
            )
        })?;
        let repository = crate::git::at(&source).identity()?.display().to_string();
        Ok(crate::claim::normalize(claims)?
            .into_iter()
            .map(|path| crate::occupancy::Subject::Surface {
                repository: repository.clone(),
                path,
            })
            .collect())
    }

    pub fn touch_issue(
        &self,
        issue: &Anchor,
        operator: Option<&crate::activity::Operator>,
        operation: &str,
    ) -> Result<crate::activity::IssueActivity> {
        self.identify(&self.identity)?;
        crate::activity::record_issue(&self.layout.root, issue, operator, operation)
    }

    pub fn activity(&self, issue: &Anchor) -> Result<crate::activity::Snapshot> {
        crate::activity::snapshot(&self.layout.root, issue)
    }

    pub fn occupancy(&self) -> Result<crate::occupancy::Snapshot> {
        crate::occupancy::inspect(&self.layout.root.join("occupancy/ledger.json"))
    }

    fn guard(&self) -> Result<File> {
        let file = self.latch()?;
        radicula_lock::lock(&file, Mode::Exclusive)?;
        Ok(file)
    }

    fn share(&self) -> Result<File> {
        let file = self.latch()?;
        radicula_lock::lock(&file, Mode::Shared)?;
        Ok(file)
    }

    fn latch(&self) -> Result<File> {
        self.identify(&self.identity)?;
        let path = self.layout.locks().join("estate.lock");
        if path.symlink_metadata().is_ok() {
            layout::direct(&path, false)?;
        }
        let file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&path)?;
        at(&path).mode(0o600)?;
        Ok(file)
    }
}

fn fault(error: keel::adapt::Error) -> Error {
    Error::typed("concord.estate.error", told(&error))
}

fn upgrade(error: keel::adapt::Error) -> Error {
    Error::typed(
        "concord.estate.upgrade_required",
        format!(
            "{}; this Concord release opens only its exact Issue estate",
            told(&error)
        ),
    )
}

fn told(error: &keel::adapt::Error) -> String {
    match error {
        keel::adapt::Error::Store(store) => format!("{store}: {}", store.detail()),
        other => other.to_string(),
    }
}

#[cfg(test)]
mod tests;
