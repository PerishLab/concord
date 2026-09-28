use crate::activity::Operator;
use crate::path::at;
use crate::{Error, Result};
use fs2::FileExt;
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

pub const LEASE: u64 = 2 * 60 * 60;
const CAPACITY: usize = 64;

#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum Subject {
    Issue { node: String },
    IssueMember { node: String, member: String },
    Surface { repository: String, path: String },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Holder {
    pub agent: crate::activity::Agent,
    pub session: String,
    pub operation: String,
    pub subjects: Vec<Subject>,
    pub heartbeat: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Conflict {
    pub holder: Holder,
    pub subjects: Vec<Subject>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Occupancy {
    pub current: Holder,
    pub conflicts: Vec<Conflict>,
    pub lease: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Observation {
    pub holder: Holder,
    pub fresh: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Snapshot {
    pub observations: Vec<Observation>,
    pub lease: u64,
    pub observed_at: u64,
}

#[derive(Debug, Deserialize, Serialize)]
struct Ledger {
    version: u32,
    holders: Vec<Holder>,
}

struct Entry<'a> {
    operator: &'a Operator,
    operation: &'a str,
    subjects: &'a [Subject],
    time: u64,
}

pub(crate) fn record(
    space: &Path,
    operator: &Operator,
    operation: &str,
    subjects: &[Subject],
) -> Result<Occupancy> {
    let time = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| Error::typed("concord.occupancy.clock", error.to_string()))?
        .as_secs();
    persist(
        space,
        Entry {
            operator,
            operation,
            subjects,
            time,
        },
    )
}

fn persist(space: &Path, entry: Entry<'_>) -> Result<Occupancy> {
    validate(entry.operator, entry.operation, entry.subjects)?;
    let root = space.join(".concord/occupancy");
    at(&root).directory()?;
    let guard = guard(&root)?;
    let path = root.join("ledger.json");
    let mut ledger = read(&path)?;
    let earliest = entry.time.saturating_sub(LEASE);
    ledger.holders.retain(|holder| holder.heartbeat >= earliest);
    let current = Holder {
        agent: entry.operator.agent,
        session: entry.operator.session.clone(),
        operation: entry.operation.to_string(),
        subjects: normalized(entry.subjects),
        heartbeat: entry.time,
    };
    let conflicts = ledger
        .holders
        .iter()
        .filter(|holder| holder.agent != current.agent || holder.session != current.session)
        .filter_map(|holder| {
            let subjects = intersections(&current.subjects, &holder.subjects);
            (!subjects.is_empty()).then(|| Conflict {
                holder: holder.clone(),
                subjects,
            })
        })
        .collect();
    ledger
        .holders
        .retain(|holder| holder.agent != current.agent || holder.session != current.session);
    ledger.holders.push(current.clone());
    ledger.holders.sort_by(|left, right| {
        right
            .heartbeat
            .cmp(&left.heartbeat)
            .then_with(|| left.agent.cmp(&right.agent))
            .then_with(|| left.session.cmp(&right.session))
    });
    ledger.holders.truncate(CAPACITY);
    let encoded = serde_json::to_vec_pretty(&ledger)
        .map_err(|error| Error::typed("concord.occupancy.encode", error.to_string()))?;
    at(&path).write(&encoded, 0o600)?;
    FileExt::unlock(&guard).map_err(|error| {
        Error::typed(
            "concord.occupancy.unlock",
            format!("cannot release occupancy ledger: {error}"),
        )
    })?;
    Ok(Occupancy {
        current,
        conflicts,
        lease: LEASE,
    })
}

fn normalized(subjects: &[Subject]) -> Vec<Subject> {
    let mut subjects = subjects.to_vec();
    subjects.sort();
    subjects.dedup();
    subjects
}

fn intersections(left: &[Subject], right: &[Subject]) -> Vec<Subject> {
    let mut found = Vec::new();
    for left in left {
        for right in right {
            let overlap = match (left, right) {
                (
                    Subject::Surface {
                        repository: one,
                        path: left,
                    },
                    Subject::Surface {
                        repository: two,
                        path: right,
                    },
                ) if one == two => crate::claim::intersections(
                    std::slice::from_ref(left),
                    std::slice::from_ref(right),
                )
                .into_iter()
                .next()
                .map(|path| Subject::Surface {
                    repository: one.clone(),
                    path,
                }),
                _ if left == right => Some(left.clone()),
                _ => None,
            };
            if let Some(overlap) = overlap {
                found.push(overlap);
            }
        }
    }
    found.sort();
    found.dedup();
    found
}

fn validate(operator: &Operator, operation: &str, subjects: &[Subject]) -> Result<()> {
    if !operator.valid() {
        return Err(Error::typed(
            "concord.occupancy.operator",
            "occupancy requires a valid agent session identity",
        ));
    }
    if operation.is_empty() || operation.len() > 128 {
        return Err(Error::typed(
            "concord.occupancy.operation",
            "occupancy operation must be 1..=128 bytes",
        ));
    }
    if subjects.is_empty() {
        return Err(Error::typed(
            "concord.occupancy.subjects",
            "occupancy requires at least one managed write subject",
        ));
    }
    Ok(())
}

fn guard(root: &Path) -> Result<File> {
    let path = root.join("ledger.lock");
    let file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&path)?;
    at(&path).mode(0o600)?;
    file.try_lock_exclusive().map_err(|error| {
        Error::typed(
            "concord.occupancy.busy",
            format!("occupancy ledger is busy: {error}"),
        )
    })?;
    Ok(file)
}

fn read(path: &Path) -> Result<Ledger> {
    if !path.is_file() {
        return Ok(Ledger {
            version: 1,
            holders: Vec::new(),
        });
    }
    let bytes = std::fs::read(path)?;
    let ledger = serde_json::from_slice::<Ledger>(&bytes)
        .map_err(|error| Error::typed("concord.occupancy.invalid", error.to_string()))?;
    if ledger.version != 1 {
        return Err(Error::typed(
            "concord.occupancy.version",
            format!("unsupported occupancy version {}", ledger.version),
        ));
    }
    Ok(ledger)
}

pub(crate) fn readable(space: &Path) -> Result<()> {
    let path = space.join(".concord/occupancy/ledger.json");
    let ledger = read(&path)?;
    for holder in &ledger.holders {
        validate(&holder.operator(), &holder.operation, &holder.subjects)?;
    }
    Ok(())
}

pub(crate) fn inspect(path: &Path) -> Result<Snapshot> {
    let observed_at = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| Error::typed("concord.occupancy.clock", error.to_string()))?
        .as_secs();
    let earliest = observed_at.saturating_sub(LEASE);
    let ledger = read(path)?;
    Ok(Snapshot {
        observations: ledger
            .holders
            .into_iter()
            .map(|holder| Observation {
                fresh: holder.heartbeat >= earliest,
                holder,
            })
            .collect(),
        lease: LEASE,
        observed_at,
    })
}

impl Holder {
    fn operator(&self) -> Operator {
        Operator {
            agent: self.agent,
            session: self.session.clone(),
        }
    }
}
