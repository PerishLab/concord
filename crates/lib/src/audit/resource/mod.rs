mod import;
mod measure;

use crate::TaskRef;
use serde::Serialize;
use std::path::Path;

pub(crate) use import::{ensure_expansion_headroom, preflight as import_preflight};

const MIB: u64 = 1024 * 1024;
const GIB: u64 = 1024 * MIB;
const TASK_WARN: u64 = 2 * GIB;
const TASK_CRIT: u64 = 8 * GIB;
const SEAT_WARN: u64 = 512 * MIB;
const SEAT_CRIT: u64 = 2 * GIB;
const USED_WARN: u64 = 60;
const USED_CRIT: u64 = 75;
const MEMORY_WARN: u64 = 40;
const MEMORY_CRIT: u64 = 25;
const MINIMUM_RESERVE: u64 = GIB;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum Status {
    Ok,
    Warn,
    Crit,
    Unknown,
}

impl Status {
    pub fn label(self) -> &'static str {
        match self {
            Self::Ok => "OK",
            Self::Warn => "WARN",
            Self::Crit => "CRIT",
            Self::Unknown => "UNKNOWN",
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct Footprint {
    pub name: String,
    pub path: String,
    pub status: Status,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bytes: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entries: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct Filesystem {
    pub status: Status,
    pub total_bytes: u64,
    pub available_bytes: u64,
    pub used_percent: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inodes: Option<Inodes>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inode_note: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct Inodes {
    pub status: Status,
    pub total: u64,
    pub available: u64,
    pub used_percent: u64,
}

#[derive(Clone, Debug, Serialize)]
pub struct HostMemory {
    pub status: Status,
    pub total_bytes: u64,
    pub available_bytes: u64,
    pub available_percent: u64,
    pub swap_total_bytes: u64,
    pub swap_used_bytes: u64,
}

#[derive(Clone, Debug, Serialize)]
pub struct Observation<T> {
    pub status: Status,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct TaskResources {
    pub identity: String,
    pub status: Status,
    pub task: Footprint,
    pub members: Vec<Footprint>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub memory: Option<Footprint>,
    pub resources: Vec<Footprint>,
    pub filesystem: Observation<Filesystem>,
    pub host_memory: Observation<HostMemory>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ImportPreflight {
    pub source: String,
    pub target: String,
    pub logical_bytes: u64,
    pub required_bytes: u64,
    pub entries: u64,
    pub available_bytes: u64,
    pub reserve_bytes: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub available_inodes: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reserve_inodes: Option<u64>,
}

pub(crate) fn host() -> Observation<HostMemory> {
    measure::host_memory()
}

pub(crate) fn inspect(task: &TaskRef, host_memory: &Observation<HostMemory>) -> TaskResources {
    let task_footprint = footprint("task", &task.path(), TASK_WARN, TASK_CRIT);
    let members = task
        .task()
        .repo
        .iter()
        .map(|member| {
            footprint(
                &format!("member:{}", member.name),
                &task.member_path(&member.name),
                TASK_WARN,
                TASK_CRIT,
            )
        })
        .collect::<Vec<_>>();
    let memory_root = task.path().join(".task");
    let memory = memory_root
        .exists()
        .then(|| footprint("memory", &memory_root, SEAT_WARN, SEAT_CRIT));
    let resources_root = memory_root.join("resources");
    let resources = match measure::child_directories(&resources_root) {
        Ok(paths) => paths
            .into_iter()
            .map(|path| {
                let name = path
                    .file_name()
                    .map(|name| name.to_string_lossy())
                    .unwrap_or_default();
                footprint(&format!("resource:{name}"), &path, SEAT_WARN, SEAT_CRIT)
            })
            .collect::<Vec<_>>(),
        Err(error) => vec![unknown("resources", &resources_root, error.to_string())],
    };
    let filesystem = measure::filesystem(&task.path());
    let mut status = task_footprint.status;
    for footprint in members.iter().chain(memory.iter()).chain(resources.iter()) {
        status = worst(status, footprint.status);
    }
    TaskResources {
        identity: task.identity(),
        status,
        task: task_footprint,
        members,
        memory,
        resources,
        filesystem,
        host_memory: host_memory.clone(),
    }
}

fn footprint(name: &str, path: &Path, warn: u64, crit: u64) -> Footprint {
    match measure::scan(path) {
        Ok(scan) => Footprint {
            name: name.to_string(),
            path: path.display().to_string(),
            status: high_status(scan.bytes, warn, crit),
            bytes: Some(scan.bytes),
            entries: Some(scan.entries),
            note: None,
        },
        Err(error) => unknown(name, path, error.to_string()),
    }
}

fn unknown(name: &str, path: &Path, note: String) -> Footprint {
    Footprint {
        name: name.to_string(),
        path: path.display().to_string(),
        status: Status::Unknown,
        bytes: None,
        entries: None,
        note: Some(note),
    }
}

fn high_status(value: u64, warn: u64, crit: u64) -> Status {
    if value >= crit {
        Status::Crit
    } else if value >= warn {
        Status::Warn
    } else {
        Status::Ok
    }
}

fn used_status(used: u64) -> Status {
    high_status(used, USED_WARN, USED_CRIT)
}

fn used_percent(total: u64, available: u64) -> u64 {
    if total == 0 {
        return 100;
    }
    total.saturating_sub(available).saturating_mul(100) / total
}

fn worst(left: Status, right: Status) -> Status {
    if rank(right) > rank(left) {
        right
    } else {
        left
    }
}

fn rank(status: Status) -> u8 {
    match status {
        Status::Ok => 0,
        Status::Unknown => 1,
        Status::Warn => 2,
        Status::Crit => 3,
    }
}
