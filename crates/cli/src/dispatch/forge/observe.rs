use concord_core::{Coordinate, Error, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::process::Stdio;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::process::Command;

const LIMIT: usize = 16 * 1024;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Observed {
    pub node: String,
    #[serde(flatten)]
    pub coordinate: Coordinate,
    pub url: String,
    pub state: String,
    pub kind: String,
    pub updated: String,
    pub seen: u64,
}

#[derive(Deserialize)]
struct Reply {
    node: String,
    number: i64,
    url: String,
    state: String,
    kind: String,
    #[serde(rename = "updated_at")]
    updated: String,
}

pub async fn issue(coordinate: &Coordinate, command: &Path, timeout: u64) -> Result<Observed> {
    let query = "query($owner:String!,$name:String!,$number:Int!){repository(owner:$owner,name:$name){issue(number:$number){id number url state updatedAt issueType{name}}}}";
    let selector = ".data.repository.issue | if . == null then null else {node: .id, number: .number, url: .url, state: .state, kind: (.issueType.name // \"\"), updated_at: .updatedAt} end";
    let mut process = Command::new(command);
    process
        .args(["api", "graphql", "-f"])
        .arg(format!("query={query}"))
        .args(["-f", &format!("owner={}", coordinate.owner)])
        .args(["-f", &format!("name={}", coordinate.repository)])
        .args(["-F", &format!("number={}", coordinate.number)])
        .args(["--jq", selector])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true);
    let output = match tokio::time::timeout(Duration::from_secs(timeout), process.output()).await {
        Ok(Ok(output)) => output,
        Ok(Err(error)) => return Err(fault("command", error.to_string())),
        Err(_) => return Err(fault("timeout", "GitHub Issue observation timed out")),
    };
    if !output.status.success() {
        return Err(fault(
            "provider",
            "GitHub Issue observation was refused by the provider",
        ));
    }
    if output.stdout.len() > LIMIT {
        return Err(fault(
            "malformed",
            "GitHub Issue observation exceeds the reply limit",
        ));
    }
    let reply = match serde_json::from_slice::<Option<Reply>>(&output.stdout) {
        Ok(Some(reply)) => reply,
        Ok(None) => return Err(fault("missing", "GitHub Issue is not readable")),
        Err(_) => {
            return Err(fault("malformed", "GitHub Issue reply is not valid JSON"));
        }
    };
    shape(coordinate, reply)
}

fn shape(coordinate: &Coordinate, reply: Reply) -> Result<Observed> {
    let expected = format!(
        "https://github.com/{}/{}/issues/{}",
        coordinate.owner, coordinate.repository, coordinate.number
    );
    if reply.number != coordinate.number || reply.url != expected {
        return Err(fault(
            "coordinate",
            "GitHub Issue reply does not match the requested coordinate",
        ));
    }
    if reply.node.trim().is_empty() {
        return Err(fault("node", "GitHub Issue has no stable node identity"));
    }
    if reply.kind.trim().is_empty() {
        return Err(fault("type", "GitHub Issue has no enabled native type"));
    }
    let state = reply.state.to_ascii_lowercase();
    if !matches!(state.as_str(), "open" | "closed") || reply.updated.is_empty() {
        return Err(fault(
            "malformed",
            "GitHub Issue reply has an invalid state or update time",
        ));
    }
    Ok(Observed {
        node: reply.node,
        coordinate: coordinate.clone(),
        url: reply.url,
        state,
        kind: reply.kind,
        updated: reply.updated,
        seen: now(),
    })
}

fn fault(kind: &str, message: impl Into<String>) -> Error {
    Error::typed(format!("concord.issue.observe.{kind}"), message.into())
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
