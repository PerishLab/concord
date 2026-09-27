use crate::path::at;
use crate::{Error, Result};
use fs2::FileExt;
use keel::adapt::db::Sqlite;
use keel::{Core, Status};
use std::fs::File;
use std::path::{Path, PathBuf};

#[path = "data/agreement.rs"]
mod agreement;
#[path = "current/brief.rs"]
mod brief;
mod current;
mod data;
mod dependency;
mod forge;
mod graph;
mod model;
mod phase;
mod task;
#[path = "data/territory.rs"]
mod territory;
mod work;

pub use agreement::{Agreement, Finding};
pub use brief::{
    FactBrief, RoleBrief, TASK_BRIEF_LIMIT, TASK_BRIEF_ROLE_BYTES, TaskBriefEntry, TaskBriefLimits,
    TaskBriefPage, TextPreview,
};
pub(in crate::estate) use data::World;
pub use data::{
    Current, Cut, Degree, Edge, Edit, Entry, Fact, Finish, Flow, Graph, Life, Link, Node, Origin,
    Part, Patch, Phase, Realm, Role, Settle, Settlement, Tune, Weight,
};
pub use forge::{Admission, Anchor, Coordinate, Reconcile};
pub use forge::{ForgeDeclaration, ForgeWithdrawal, Reference, ReferenceKind};
pub use model::migration;
pub use task::{Annotate, Rehome, Rename, Repository, Retire};
pub use work::landing;
pub use work::{
    Artifact, Attach, BoundaryState, CheckoutState, ClaimOverlap, Claiming, Import,
    IntegrationState, MemberChange, MemberStatus, Narrowing, Proof, Proving, Release, Removal,
    Retirement, Survey, UpstreamState, Worktree,
};

pub struct Estate {
    core: Core<Sqlite>,
    space: PathBuf,
    anchors: bool,
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
            anchors: true,
        };
        estate.seed().await?;
        estate.verify().await?;
        Ok(estate)
    }

    pub async fn open(&self) -> Result<Estate> {
        if !self.database().is_file() || !self.sudo().is_file() {
            return Err(Error::typed(
                "concord.estate.absent",
                format!(
                    "Concord estate is absent at {}; bootstrap a new Space or follow the v0.11.0 release migration contract for a legacy Space",
                    self.root.display()
                ),
            ));
        }
        let sudo = std::fs::read_to_string(self.sudo())?;
        let current = self.bind(model::graph, &sudo).await;
        let (core, anchors) = match current {
            Ok(core) => (core, true),
            Err(primary) => match self.bind(model::released, &sudo).await {
                Ok(core) => (core, false),
                Err(_) => return Err(primary),
            },
        };
        let estate = Estate {
            core,
            space: self.space.clone(),
            anchors,
        };
        estate.verify().await?;
        Ok(estate)
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

    pub fn touch(
        &self,
        task: &Node,
        operator: Option<&crate::activity::Operator>,
        operation: &str,
    ) -> Result<crate::activity::Activity> {
        crate::activity::record(&self.space, task, operator, operation)
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

    async fn seed(&self) -> Result<()> {
        let rows = self.core.live("Space").await.map_err(fault)?;
        if rows.is_empty() {
            self.core
                .put("Space", &[("name", "space"), ("revision", "0")])
                .await
                .map_err(fault)?;
        }
        Ok(())
    }

    async fn verify(&self) -> Result<()> {
        let rows = self.core.live("Space").await.map_err(fault)?;
        if rows.len() != 1 {
            return Err(Error::typed(
                "concord.estate.space",
                format!(
                    "Concord estate needs exactly one Space, found {}",
                    rows.len()
                ),
            ));
        }
        Ok(())
    }
}

fn fault(error: keel::adapt::Error) -> Error {
    Error::typed("concord.estate.error", error.to_string())
}

fn upgrade(error: keel::adapt::Error) -> Error {
    Error::typed(
        "concord.estate.upgrade_required",
        format!("{error}; follow the exact migration contract for the target Concord release"),
    )
}
