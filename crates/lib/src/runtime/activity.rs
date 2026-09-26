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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent: Option<Agent>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session: Option<String>,
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
            version: 2,
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
    pub fn detect() -> Option<Self> {
        crate::config::operator()
    }

    pub(crate) fn valid(&self) -> bool {
        let session = self.session.as_str();
        !session.is_empty()
            && session.len() <= 512
            && session.chars().all(|character| {
                character.is_ascii_alphanumeric()
                    || matches!(character, '-' | '_' | '.' | ':' | '/')
            })
    }
}

pub(crate) fn record(
    space: &Path,
    task: &Node,
    operator: Option<&Operator>,
    operation: &str,
) -> Result<Activity> {
    validate(operation)?;
    let operator = operator.filter(|operator| operator.valid());
    let time = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| Error::typed("concord.activity.clock", error.to_string()))?
        .as_secs();
    let root = space.join(".concord/activity");
    at(&root).directory()?;
    let guard = guard(&root, task.key)?;
    let path = root.join(format!("{}.json", task.key));
    let mut ledger = read(&path)?;
    ledger.version = 2;
    let identity = task.identity();
    let current = Touch {
        agent: operator.map(|operator| operator.agent),
        session: operator.map(|operator| operator.session.clone()),
        operation: operation.to_string(),
        time,
    };
    let previous = operator.and_then(|operator| {
        ledger
            .touches
            .iter()
            .filter(|touch| {
                touch.agent == Some(operator.agent)
                    && touch.session.as_deref() == Some(operator.session.as_str())
            })
            .map(|touch| touch.time)
            .max()
    });
    if let Some(operator) = operator {
        ledger.touches.retain(|touch| {
            touch.agent != Some(operator.agent)
                || touch.session.as_deref() != Some(operator.session.as_str())
        });
    }
    ledger.touches.push(current.clone());
    ledger.touches.sort_by(|left, right| {
        right
            .time
            .cmp(&left.time)
            .then_with(|| right.agent.cmp(&left.agent))
            .then_with(|| right.session.cmp(&left.session))
    });
    ledger.touches.truncate(CAPACITY);
    let recent = recent(&ledger, operator, previous, time);
    let encoded = serde_json::to_vec_pretty(&ledger)
        .map_err(|error| Error::typed("concord.activity.encode", error.to_string()))?;
    at(&path).write(&encoded, 0o600)?;
    let activity = Activity {
        task: identity,
        current,
        recent,
        window: WINDOW,
    };
    FileExt::unlock(&guard).map_err(|error| {
        Error::typed(
            "concord.activity.unlock",
            format!("cannot release Task activity ledger: {error}"),
        )
    })?;
    Ok(activity)
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
            format!("Task activity ledger is busy: {error}"),
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
    if !matches!(ledger.version, 1 | 2) {
        return Err(Error::typed(
            "concord.activity.version",
            format!("unsupported Task activity version {}", ledger.version),
        ));
    }
    Ok(ledger)
}

fn recent(
    ledger: &Ledger,
    operator: Option<&Operator>,
    previous: Option<u64>,
    time: u64,
) -> Vec<Touch> {
    let Some(operator) = operator else {
        return Vec::new();
    };
    let earliest = time.saturating_sub(WINDOW);
    ledger
        .touches
        .iter()
        .filter(|touch| touch.time >= earliest)
        .filter(|touch| previous.is_none_or(|previous| touch.time > previous))
        .filter(|touch| match (touch.agent, touch.session.as_deref()) {
            (Some(agent), Some(session)) => agent != operator.agent || session != operator.session,
            _ => false,
        })
        .take(LIMIT)
        .cloned()
        .collect()
}

fn validate(operation: &str) -> Result<()> {
    if operation.is_empty() || operation.len() > 128 {
        return Err(Error::typed(
            "concord.activity.operation",
            "Task activity operation must be 1..=128 bytes",
        ));
    }
    Ok(())
}
