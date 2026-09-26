use concord_core::{Reference, ReferenceKind};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::process::Stdio;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::process::Command;

const LIMIT: usize = 16 * 1024;

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(tag = "availability", rename_all = "kebab-case")]
pub enum Observation {
    Available {
        #[serde(flatten)]
        reference: Reference,
        node: String,
        url: String,
        state: String,
        provider_updated_at: String,
        observed_at: u64,
        freshness_seconds: u64,
    },
    Unavailable {
        #[serde(skip_serializing_if = "Option::is_none")]
        reference: Option<Reference>,
        observed_at: u64,
        reason: Reason,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Reason {
    Reference,
    Command,
    Timeout,
    Provider,
    Missing,
    Malformed,
}

#[derive(Deserialize)]
struct Reply {
    node: String,
    url: String,
    state: String,
    #[serde(rename = "updated_at")]
    updated: String,
    #[serde(rename = "updated_at_epoch")]
    epoch: u64,
}

pub async fn observe(
    reference: Option<&Reference>,
    command: Option<&Path>,
    timeout: u64,
) -> Observation {
    let Some(reference) = reference else {
        return unavailable(None, Reason::Reference);
    };
    let Some(command) = command else {
        return unavailable(Some(reference.clone()), Reason::Command);
    };
    let (query, selector) = request(reference.kind);
    let mut process = Command::new(command);
    process
        .args(["api", "graphql", "-f"])
        .arg(format!("query={query}"))
        .args(["-f", &format!("owner={}", reference.owner)])
        .args(["-f", &format!("name={}", reference.repository)])
        .args(["-F", &format!("number={}", reference.number)])
        .args(["--jq", selector])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true);
    let output = match tokio::time::timeout(Duration::from_secs(timeout), process.output()).await {
        Ok(Ok(output)) => output,
        Ok(Err(_)) => return unavailable(Some(reference.clone()), Reason::Command),
        Err(_) => return unavailable(Some(reference.clone()), Reason::Timeout),
    };
    if !output.status.success() {
        return unavailable(Some(reference.clone()), Reason::Provider);
    }
    if output.stdout.len() > LIMIT {
        return unavailable(Some(reference.clone()), Reason::Malformed);
    }
    let reply = match serde_json::from_slice::<Option<Reply>>(&output.stdout) {
        Ok(Some(reply)) => reply,
        Ok(None) => return unavailable(Some(reference.clone()), Reason::Missing),
        Err(_) => return unavailable(Some(reference.clone()), Reason::Malformed),
    };
    available(reference, reply)
        .unwrap_or_else(|| unavailable(Some(reference.clone()), Reason::Malformed))
}

pub fn print(observation: &Observation) {
    match observation {
        Observation::Available {
            reference,
            node,
            state,
            freshness_seconds,
            ..
        } => println!(
            "  provider observation: {} {}/{}#{} node={} state={} freshness={}s",
            reference.provider,
            reference.owner,
            reference.repository,
            reference.number,
            node,
            state,
            freshness_seconds
        ),
        Observation::Unavailable { reason, .. } => {
            println!("  provider observation: unavailable ({})", reason.name())
        }
    }
}

impl Reason {
    fn name(self) -> &'static str {
        match self {
            Self::Reference => "reference",
            Self::Command => "command",
            Self::Timeout => "timeout",
            Self::Provider => "provider",
            Self::Missing => "missing",
            Self::Malformed => "malformed",
        }
    }
}

fn available(reference: &Reference, reply: Reply) -> Option<Observation> {
    let path = match reference.kind {
        ReferenceKind::Issue => "/issues/",
        ReferenceKind::Change => "/pull/",
    };
    let prefix = format!(
        "https://github.com/{}/{}{}",
        reference.owner, reference.repository, path
    );
    let state = reply.state.to_ascii_lowercase();
    let valid = match reference.kind {
        ReferenceKind::Issue => matches!(state.as_str(), "open" | "closed"),
        ReferenceKind::Change => matches!(state.as_str(), "open" | "closed" | "merged"),
    };
    if reply.node.is_empty() || !reply.url.starts_with(&prefix) {
        return None;
    }
    if reply.updated.is_empty() || reply.epoch == 0 {
        return None;
    }
    if !valid {
        return None;
    }
    let observed = now();
    Some(Observation::Available {
        reference: reference.clone(),
        node: reply.node,
        url: reply.url,
        state,
        provider_updated_at: reply.updated,
        observed_at: observed,
        freshness_seconds: observed.saturating_sub(reply.epoch),
    })
}

fn unavailable(reference: Option<Reference>, reason: Reason) -> Observation {
    Observation::Unavailable {
        reference,
        observed_at: now(),
        reason,
    }
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn request(kind: ReferenceKind) -> (&'static str, &'static str) {
    match kind {
        ReferenceKind::Issue => (
            "query($owner:String!,$name:String!,$number:Int!){repository(owner:$owner,name:$name){issue(number:$number){id url state updatedAt}}}",
            ".data.repository.issue | if . == null then null else {node: .id, url: .url, state: .state, updated_at: .updatedAt, updated_at_epoch: (.updatedAt | fromdateiso8601)} end",
        ),
        ReferenceKind::Change => (
            "query($owner:String!,$name:String!,$number:Int!){repository(owner:$owner,name:$name){pullRequest(number:$number){id url state updatedAt}}}",
            ".data.repository.pullRequest | if . == null then null else {node: .id, url: .url, state: .state, updated_at: .updatedAt, updated_at_epoch: (.updatedAt | fromdateiso8601)} end",
        ),
    }
}
