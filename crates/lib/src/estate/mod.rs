use crate::path::at;
use crate::{Error, Result};
use fs2::FileExt;
use keel::adapt::db::Sqlite;
use keel::{Core, Status};
use std::fs::File;
use std::path::{Path, PathBuf};

#[path = "data/agreement.rs"]
mod agreement;
mod forge;
mod model;
#[path = "data/territory/mod.rs"]
mod territory;
mod work;

pub use agreement::{Agreement, Finding};
pub use forge::{Admission, Anchor, Coordinate, Reconcile};
pub use forge::{Reference, ReferenceKind};
pub use work::{
    BoundaryState, CheckoutState, ClaimOverlap, IntegrationState, IssueArtifact, IssueAttach,
    IssueClaiming, IssueDeclaration, IssueImport, IssueMemberChange, IssueMemberStatus,
    IssueNarrowing, IssueProving, IssueRelease, IssueRemoval, IssueRetirement, IssueWithdrawal,
    IssueWorktree, Proof, UpstreamState, authority, issue_delivery, issue_landing,
};

pub struct Estate {
    core: Core<Sqlite>,
    space: PathBuf,
}

#[derive(Clone, Debug)]
pub struct Seat {
    space: PathBuf,
    root: PathBuf,
}

impl Seat {
    pub fn new(space: impl AsRef<Path>) -> Self {
        let space = space.as_ref().to_path_buf();
        Self {
            root: space.join(".concord"),
            space,
        }
    }

    pub fn database(&self) -> PathBuf {
        self.root.join("estate.sqlite3")
    }

    pub fn sudo(&self) -> PathBuf {
        self.root.join("sudo")
    }

    pub async fn bootstrap(&self) -> Result<Estate> {
        at(&self.root).directory()?;
        let wire = Sqlite::file(self.database()).await.map_err(fault)?;
        let mut held = keel::bootstrap(model::graph(), wire).map_err(fault)?;
        let status = held.status().await.map_err(fault)?;
        let sudo = self.possession(&mut held, status).await?;
        let core = held.seal(&sudo).await.map_err(fault)?;
        at(&self.database()).mode(0o600)?;
        let estate = Estate {
            core,
            space: self.space.clone(),
        };
        Ok(estate)
    }

    pub async fn open(&self) -> Result<Estate> {
        if !self.database().is_file() || !self.sudo().is_file() {
            return Err(Error::typed(
                "concord.estate.absent",
                format!(
                    "Concord estate is absent at {}; bootstrap a fresh Issue estate",
                    self.root.display()
                ),
            ));
        }
        let sudo = std::fs::read_to_string(self.sudo())?;
        let core = self.bind(model::graph, &sudo).await?;
        Ok(Estate {
            core,
            space: self.space.clone(),
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
    pub fn occupy(
        &self,
        operator: &crate::activity::Operator,
        operation: &str,
        subjects: &[crate::occupancy::Subject],
    ) -> Result<crate::occupancy::Occupancy> {
        crate::occupancy::record(&self.space, operator, operation, subjects)
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
        crate::activity::record_issue(&self.space, issue, operator, operation)
    }

    pub fn activity(&self, issue: &Anchor) -> Result<crate::activity::Snapshot> {
        crate::activity::snapshot(&self.space, issue)
    }

    pub fn occupancy(&self) -> Result<crate::occupancy::Snapshot> {
        crate::occupancy::inspect(&self.space.join(".concord/occupancy/ledger.json"))
    }

    fn guard(&self) -> Result<File> {
        let path = self.space.join(".concord.lock");
        let file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&path)?;
        at(&path).mode(0o600)?;
        file.lock_exclusive()?;
        Ok(file)
    }
}

fn fault(error: keel::adapt::Error) -> Error {
    Error::typed("concord.estate.error", error.to_string())
}

fn upgrade(error: keel::adapt::Error) -> Error {
    Error::typed(
        "concord.estate.upgrade_required",
        format!("{error}; this Concord release opens only its exact Issue estate"),
    )
}
