mod audit;
mod config;
#[path = "runtime/error.rs"]
mod error;
#[path = "runtime/git.rs"]
mod git;
mod memory;
#[path = "runtime/observation.rs"]
pub mod observation;
mod ops;
#[path = "runtime/path.rs"]
mod path;
mod protocol;
mod store;

pub(crate) use protocol::{boundary, claim, model};

pub use audit::{
    Advisory, Audit, Fault, Filesystem, Footprint, HostMemory, ImportPreflight, Inodes,
    LandingProof, MemberPreflight, MemberProof, Observation, Preflight, Status, TaskResources,
};
pub use config::Root;
pub use error::{Error, Result};
pub use memory::{
    MAX_MAIN_BYTES, MAX_MAIN_LINES, MAX_PHASE_BYTES, MAX_PHASE_LINES, MAX_RAW_READ_BYTES, Memory,
    MemoryBrief, MemoryChange, MemoryRead, PhaseEntry, TextPreview,
};
pub use ops::{
    Action, Add, MigrationClaim, Plan, TASK_BRIEF_LIMIT, TASK_BRIEF_SECTION_BYTES, TaskBriefEntry,
    TaskBriefLimits, TaskBriefPage,
};
pub use protocol::{BoundaryCheck, BoundaryProof, Member, PLUMB_VERSION, Registry, Repo, Task};
pub use store::{Domain, Space, TaskRef};
