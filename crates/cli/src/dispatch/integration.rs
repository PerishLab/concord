use super::emit;
use crate::args::integration::Command as IntegrationCommand;
use concord_core::{Error, Estate, Register, Rename, Repository, Result};
use serde::Deserialize;
use serde_json::json;
use std::path::Path;
use std::process::Stdio;
use std::time::Duration;
use tokio::process::Command;

const LIMIT: usize = 16 * 1024;

#[derive(Deserialize)]
struct Observed {
    node: String,
    coordinate: String,
    branch: String,
}

pub async fn run(estate: &Estate, command: IntegrationCommand, output: bool) -> Result<()> {
    match command {
        IntegrationCommand::List => emit(
            json!({"integrations": estate.integrations().await?}),
            output,
        ),
        IntegrationCommand::Register {
            repository,
            path,
            command,
            timeout,
        } => {
            let repository = Repository::parse(&repository)?;
            let observed = observe(&repository, &command, timeout).await?;
            let integration = estate
                .register(&Register {
                    node: observed.node,
                    repository,
                    path,
                })
                .await?;
            emit(json!({"integration": integration}), output)
        }
        IntegrationCommand::Reconcile {
            repository,
            command,
            timeout,
        } => {
            let repository = Repository::parse(&repository)?;
            let observed = observe(&repository, &command, timeout).await?;
            let integration = estate
                .rename(&Rename {
                    node: observed.node,
                    repository,
                })
                .await?;
            emit(json!({"integration": integration}), output)
        }
    }
}

async fn observe(repository: &Repository, command: &Path, timeout: u64) -> Result<Observed> {
    let query = "query($owner:String!,$name:String!){repository(owner:$owner,name:$name){id nameWithOwner defaultBranchRef{name}}}";
    let selector = ".data.repository | if . == null then null else {node: .id, coordinate: .nameWithOwner, branch: (.defaultBranchRef.name // \"\")} end";
    let mut process = Command::new(command);
    process
        .args(["api", "graphql", "-f"])
        .arg(format!("query={query}"))
        .args(["-f", &format!("owner={}", repository.owner)])
        .args(["-f", &format!("name={}", repository.name)])
        .args(["--jq", selector])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true);
    let output = match tokio::time::timeout(Duration::from_secs(timeout), process.output()).await {
        Ok(Ok(output)) => output,
        Ok(Err(error)) => return Err(fault("command", error.to_string())),
        Err(_) => return Err(fault("timeout", "GitHub repository observation timed out")),
    };
    if !output.status.success() {
        return Err(fault(
            "provider",
            "GitHub repository observation was refused by the provider",
        ));
    }
    if output.stdout.len() > LIMIT {
        return Err(fault(
            "malformed",
            "GitHub repository observation exceeds the reply limit",
        ));
    }
    let observed = serde_json::from_slice::<Option<Observed>>(&output.stdout)
        .map_err(|_| fault("malformed", "GitHub repository reply is not valid JSON"))?
        .ok_or_else(|| fault("missing", "GitHub repository is not readable"))?;
    if observed.node.trim().is_empty()
        || observed.coordinate != repository.identity()
        || observed.branch != "main"
    {
        return Err(fault(
            "shape",
            "GitHub repository identity or default branch does not match the request",
        ));
    }
    Ok(observed)
}

fn fault(kind: &str, message: impl Into<String>) -> Error {
    Error::typed(
        format!("concord.integration.observe.{kind}"),
        message.into(),
    )
}
