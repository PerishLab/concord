use concord_core::activity::Operator;
use concord_core::{Coordinate, Error, Result};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::io::Read;
use std::path::{Path, PathBuf};

const BODY: usize = 65_536;
const REPLY: usize = 16 * 1024;
const MUTATION: &str = "mutation($subject:ID!,$body:String!){addComment(input:{subjectId:$subject,body:$body}){commentEdge{node{id url body createdAt}}}}";
const SEMANTICS: &str =
    "execution fields are caller-environment observations, not identity or authority";

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Execution {
    pub agent: String,
    pub session: String,
    pub host: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Declared {
    pub node: String,
    pub url: String,
    pub created: String,
    pub issue: String,
    pub execution: Execution,
    pub semantics: String,
}

#[derive(Deserialize)]
struct Envelope {
    data: Option<Data>,
}

#[derive(Deserialize)]
struct Data {
    #[serde(rename = "addComment")]
    addition: Option<Addition>,
}

#[derive(Deserialize)]
struct Addition {
    #[serde(rename = "commentEdge")]
    edge: Option<Edge>,
}

#[derive(Deserialize)]
struct Edge {
    node: Reply,
}

#[derive(Deserialize)]
pub(in crate::dispatch) struct Reply {
    id: String,
    url: String,
    body: String,
    #[serde(rename = "createdAt")]
    created: String,
}

pub async fn declare(
    coordinate: &Coordinate,
    input: &Path,
    command: &Path,
    timeout: u64,
) -> Result<Declared> {
    let prose = read(input)?;
    let execution = execution()?;
    let body = render(&prose, &execution)?;
    let issue = super::observe::issue(coordinate, command, timeout).await?;
    let reply = submit(command, &issue.node, &body, timeout).await?;
    verify(coordinate, &body, &reply)?;
    Ok(Declared {
        node: reply.id,
        url: reply.url,
        created: reply.created,
        issue: coordinate.identity(),
        execution,
        semantics: SEMANTICS.to_string(),
    })
}

pub fn output(comment: Declared, json: bool) -> Result<()> {
    if json {
        return super::super::super::emit(json!({"comment": comment}), true);
    }
    println!("{}", comment.url);
    let host = comment
        .execution
        .host
        .as_ref()
        .map(|host| format!(" host={host}"))
        .unwrap_or_default();
    println!(
        "Concord observation: agent={} session={}{}; non-authoritative",
        comment.execution.agent, comment.execution.session, host
    );
    Ok(())
}

pub(in crate::dispatch) fn execution() -> Result<Execution> {
    let operator = Operator::detect().ok_or_else(|| {
        Error::typed(
            "concord.issue.comment.operator",
            "Issue comment declaration requires exactly one valid agent session",
        )
    })?;
    Ok(Execution {
        agent: operator.agent.name().to_string(),
        session: operator.session,
        host: concord_core::host()?,
    })
}

pub(in crate::dispatch) fn render(prose: &str, execution: &Execution) -> Result<String> {
    let prose = prose.trim_end();
    if prose.trim().is_empty() {
        return Err(Error::typed(
            "concord.issue.comment.empty",
            "Issue comment body must not be empty",
        ));
    }
    let host = execution
        .host
        .as_ref()
        .map(|host| format!(" · host `{host}`"))
        .unwrap_or_default();
    let marker = serde_json::to_string(execution)
        .map_err(|error| Error::typed("concord.issue.comment.encode", error.to_string()))?;
    let body = format!(
        "{prose}\n\n---\nConcord: `{}` session `{}`{host}\n\n<!-- concord.issue-comment/v1\n{marker}\n-->",
        execution.agent, execution.session
    );
    if body.len() > BODY {
        return Err(Error::typed(
            "concord.issue.comment.limit",
            format!("rendered Issue comment exceeds {BODY} bytes"),
        ));
    }
    Ok(body)
}

fn read(path: &Path) -> Result<String> {
    let bytes = if path == Path::new("-") {
        bounded(std::io::stdin().lock(), "stdin")?
    } else {
        regular(path)?
    };
    String::from_utf8(bytes).map_err(|error| {
        Error::typed(
            "concord.issue.comment.encoding",
            format!("Issue comment input must be UTF-8: {error}"),
        )
    })
}

fn regular(path: &Path) -> Result<Vec<u8>> {
    let metadata = std::fs::symlink_metadata(path).map_err(|error| {
        Error::typed(
            "concord.issue.comment.read",
            format!("cannot inspect input {}: {error}", path.display()),
        )
    })?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(Error::typed(
            "concord.issue.comment.type",
            format!("input is not a regular file: {}", path.display()),
        ));
    }
    let file = std::fs::File::open(PathBuf::from(path)).map_err(|error| {
        Error::typed(
            "concord.issue.comment.read",
            format!("cannot open input {}: {error}", path.display()),
        )
    })?;
    bounded(file, &path.display().to_string())
}

fn bounded(mut reader: impl Read, label: &str) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    reader
        .by_ref()
        .take((BODY + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|error| {
            Error::typed(
                "concord.issue.comment.read",
                format!("cannot read {label}: {error}"),
            )
        })?;
    if bytes.len() > BODY {
        return Err(Error::typed(
            "concord.issue.comment.limit",
            format!("Issue comment input exceeds {BODY} bytes"),
        ));
    }
    Ok(bytes)
}

pub(in crate::dispatch) async fn submit(
    command: &Path,
    subject: &str,
    body: &str,
    timeout: u64,
) -> Result<Reply> {
    let request = serde_json::to_vec(&json!({
        "query": MUTATION,
        "variables": {"subject": subject, "body": body},
    }))
    .map_err(|error| Error::typed("concord.issue.comment.encode", error.to_string()))?;
    let output = super::super::github::transport::Request::new(command, timeout, REPLY)
        .args(["api", "graphql", "--input", "-"])
        .input(&request)
        .run()
        .await
        .map_err(failure)?;
    let envelope = serde_json::from_slice::<Envelope>(&output.stdout)
        .map_err(|_| disagreement("GitHub comment reply is not valid JSON"))?;
    envelope
        .data
        .and_then(|data| data.addition)
        .and_then(|addition| addition.edge)
        .map(|edge| edge.node)
        .ok_or_else(|| disagreement("GitHub comment reply named no comment"))
}

fn failure(failure: super::super::github::transport::Failure) -> Error {
    use super::super::github::transport::{Failure, Stream};
    match failure {
        Failure::Spawn(_) => Error::typed("concord.issue.comment.command", failure.to_string()),
        Failure::Refused { .. } => Error::typed(
            "concord.issue.comment.provider",
            format!("GitHub refused Issue comment declaration: {failure}"),
        ),
        Failure::Oversized(Stream::Stdout) => {
            disagreement("GitHub comment reply exceeds the reply limit")
        }
        Failure::Oversized(Stream::Stderr) => Error::typed(
            "concord.issue.comment.provider",
            "GitHub comment refusal exceeds the reply limit",
        ),
        Failure::Exchange(_) | Failure::Timeout => Error::typed(
            "concord.issue.comment.indeterminate",
            format!("GitHub comment completion is indeterminate: {failure}"),
        ),
    }
}

pub(in crate::dispatch) fn verify(
    coordinate: &Coordinate,
    body: &str,
    reply: &Reply,
) -> Result<()> {
    let prefix = format!(
        "https://github.com/{}/{}/issues/{}#issuecomment-",
        coordinate.owner, coordinate.repository, coordinate.number
    );
    let identity = !reply.id.trim().is_empty() && !reply.created.trim().is_empty();
    let content = reply.url.starts_with(&prefix) && reply.body == body;
    if !identity || !content {
        return Err(disagreement(
            "GitHub comment reply disagrees with the requested Issue or rendered body",
        ));
    }
    Ok(())
}

fn disagreement(message: impl Into<String>) -> Error {
    Error::typed("concord.issue.comment.disagreement", message.into())
}
