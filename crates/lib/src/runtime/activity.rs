use crate::estate::Node;
use crate::path::at;
use crate::{Error, Result};
use fs2::FileExt;
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

const CAPACITY: usize = 64;
const LIMIT: usize = 8;
const WINDOW: u64 = 24 * 60 * 60;

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Agent {
    Claude,
    Grok,
    Codex,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Operator {
    pub agent: Agent,
    pub session: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Touch {
    pub agent: Agent,
    pub session: String,
    pub operation: String,
    pub time: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Activity {
    pub task: String,
    pub current: Touch,
    pub recent: Vec<Touch>,
    pub window: u64,
}

#[derive(Debug, Deserialize, Serialize)]
struct Ledger {
    version: u32,
    touches: Vec<Touch>,
}

impl Default for Ledger {
    fn default() -> Self {
        Self {
            version: 1,
            touches: Vec::new(),
        }
    }
}

impl Agent {
    pub fn name(self) -> &'static str {
        match self {
            Self::Claude => "claude",
            Self::Grok => "grok",
            Self::Codex => "codex",
        }
    }
}

impl Operator {
    pub fn detect() -> Result<Option<Self>> {
        crate::config::operator()
    }
}

pub(crate) fn record(
    space: &Path,
    task: &Node,
    operator: &Operator,
    operation: &str,
) -> Result<Activity> {
    validate(operator, operation)?;
    let time = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| Error::typed("concord.activity.clock", error.to_string()))?
        .as_secs();
    let root = space.join(".concord/activity");
    at(&root).directory()?;
    let _guard = guard(&root, task.key)?;
    let path = root.join(format!("{}.json", task.key));
    let mut ledger = read(&path)?;
    let identity = task.identity();
    let current = Touch {
        agent: operator.agent,
        session: operator.session.clone(),
        operation: operation.to_string(),
        time,
    };
    ledger
        .touches
        .retain(|touch| touch.agent != operator.agent || touch.session != operator.session);
    ledger.touches.push(current.clone());
    ledger.touches.sort_by(|left, right| {
        right
            .time
            .cmp(&left.time)
            .then_with(|| right.agent.cmp(&left.agent))
            .then_with(|| right.session.cmp(&left.session))
    });
    ledger.touches.truncate(CAPACITY);
    let recent = recent(&ledger, operator, time);
    let encoded = serde_json::to_vec_pretty(&ledger)
        .map_err(|error| Error::typed("concord.activity.encode", error.to_string()))?;
    at(&path).write(&encoded, 0o600)?;
    Ok(Activity {
        task: identity,
        current,
        recent,
        window: WINDOW,
    })
}

fn guard(root: &Path, key: i64) -> Result<File> {
    let path = root.join(format!("{key}.lock"));
    let file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&path)?;
    at(&path).mode(0o600)?;
    file.try_lock_exclusive().map_err(|error| {
        Error::typed(
            "concord.activity.busy",
            format!("operator activity ledger is busy: {error}"),
        )
    })?;
    Ok(file)
}

fn read(path: &Path) -> Result<Ledger> {
    if !path.is_file() {
        return Ok(Ledger::default());
    }
    let bytes = std::fs::read(path)?;
    let ledger = serde_json::from_slice::<Ledger>(&bytes)
        .map_err(|error| Error::typed("concord.activity.invalid", error.to_string()))?;
    if ledger.version != 1 {
        return Err(Error::typed(
            "concord.activity.version",
            format!("unsupported operator activity version {}", ledger.version),
        ));
    }
    Ok(ledger)
}

fn recent(ledger: &Ledger, operator: &Operator, time: u64) -> Vec<Touch> {
    let earliest = time.saturating_sub(WINDOW);
    ledger
        .touches
        .iter()
        .filter(|touch| touch.time >= earliest)
        .filter(|touch| touch.agent != operator.agent || touch.session != operator.session)
        .take(LIMIT)
        .cloned()
        .collect()
}

fn validate(operator: &Operator, operation: &str) -> Result<()> {
    let session = operator.session.as_str();
    let safe = session.chars().all(|character| {
        character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.' | ':' | '/')
    });
    if session.is_empty() || session.len() > 512 || !safe {
        return Err(Error::typed(
            "concord.activity.session",
            "operator session id must be 1..=512 safe ASCII bytes",
        ));
    }
    if operation.is_empty() || operation.len() > 128 {
        return Err(Error::typed(
            "concord.activity.operation",
            "operator activity operation must be 1..=128 bytes",
        ));
    }
    Ok(())
}
