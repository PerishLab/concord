use crate::estate::Anchor;
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
pub struct IssueActivity {
    pub issue: crate::estate::Coordinate,
    pub node: String,
    pub current: Touch,
    pub recent: Vec<Touch>,
    pub window: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Observation {
    pub touch: Touch,
    pub fresh: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Snapshot {
    pub observations: Vec<Observation>,
    pub window: u64,
    pub observed_at: u64,
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

fn persist(
    space: &Path,
    key: &str,
    operator: Option<&Operator>,
    operation: &str,
) -> Result<(Touch, Vec<Touch>)> {
    validate(operation)?;
    let operator = operator.filter(|operator| operator.valid());
    let time = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| Error::typed("concord.activity.clock", error.to_string()))?
        .as_secs();
    let root = space.join(".concord/activity");
    at(&root).directory()?;
    let guard = guard(&root, key)?;
    let path = root.join(format!("{key}.json"));
    let mut ledger = read(&path)?;
    ledger.version = 2;
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
    FileExt::unlock(&guard).map_err(|error| {
        Error::typed(
            "concord.activity.unlock",
            format!("cannot release activity ledger: {error}"),
        )
    })?;
    Ok((current, recent))
}

pub(crate) fn record_issue(
    space: &Path,
    issue: &Anchor,
    operator: Option<&Operator>,
    operation: &str,
) -> Result<IssueActivity> {
    let (current, recent) = persist(space, &format!("issue-{}", issue.key), operator, operation)?;
    Ok(IssueActivity {
        issue: issue.coordinate.clone(),
        node: issue.node.clone(),
        current,
        recent,
        window: WINDOW,
    })
}

pub(crate) fn snapshot(space: &Path, issue: &Anchor) -> Result<Snapshot> {
    let observed_at = now()?;
    let ledger = read(&space.join(format!(".concord/activity/issue-{}.json", issue.key)))?;
    let earliest = observed_at.saturating_sub(WINDOW);
    Ok(Snapshot {
        observations: ledger
            .touches
            .into_iter()
            .map(|touch| Observation {
                fresh: touch.time >= earliest,
                touch,
            })
            .collect(),
        window: WINDOW,
        observed_at,
    })
}

fn now() -> Result<u64> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| Error::typed("concord.activity.clock", error.to_string()))
        .map(|duration| duration.as_secs())
}

fn guard(root: &Path, key: &str) -> Result<File> {
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
            format!("Issue activity ledger is busy: {error}"),
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
    if ledger.version != 2 {
        return Err(Error::typed(
            "concord.activity.version",
            format!("unsupported Issue activity version {}", ledger.version),
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
            "Issue activity operation must be 1..=128 bytes",
        ));
    }
    Ok(())
}
