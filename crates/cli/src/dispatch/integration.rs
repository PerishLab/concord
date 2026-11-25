mod rebind;

use super::emit;
use crate::args::integration;
use crate::runtime::github;
use concord_core::{Error, Estate, Integration, Register, Rename, Repository, Result};
use serde_json::json;

struct Observed {
    node: String,
    coordinate: String,
    branch: String,
}

pub async fn run(estate: &Estate, command: integration::Command, output: bool) -> Result<()> {
    match command {
        integration::Command::Rebind { command } => rebind::run(estate, command, output).await,
        integration::Command::List => emit(
            json!({"integrations": estate.integrations().await?}),
            output,
        ),
        integration::Command::Retire {
            repository,
            node,
            key,
            apply,
        } => {
            let held = select(estate, &repository, &node, key).await?;
            let retired = estate.retire(&held, apply).await?;
            emit(retirement(retired, apply), output)
        }
        integration::Command::Register {
            repository,
            path,
            timeout,
        } => {
            let repository = Repository::parse(&repository)?;
            let observed = observe(&repository, timeout).await?;
            let integration = estate
                .register(&Register {
                    node: observed.node,
                    repository,
                    path,
                })
                .await?;
            emit(json!({"integration": integration}), output)
        }
        integration::Command::Reconcile {
            repository,
            timeout,
        } => {
            let repository = Repository::parse(&repository)?;
            let observed = observe(&repository, timeout).await?;
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

fn retirement(integration: Integration, applied: bool) -> serde_json::Value {
    json!({"retirement": {"integration": integration, "applied": applied}})
}

async fn select(estate: &Estate, repository: &str, node: &str, key: i64) -> Result<Integration> {
    let repository = Repository::parse(repository)?;
    estate
        .integrations()
        .await?
        .into_iter()
        .find(|held| held.key == key && held.node == node && held.repository == repository)
        .ok_or_else(|| {
            Error::typed(
                "concord.integration.changed",
                "no Integration matches the exact key, node and repository; reread integration list",
            )
        })
}

async fn observe(repository: &Repository, timeout: u64) -> Result<Observed> {
    let client = github::client(timeout).map_err(|error| fault("provider", error))?;
    let found = github::call("repository.show", || {
        client.repository(&repository.owner, &repository.name)
    })
    .map_err(|error| fault("provider", github::describe(error)))?
    .value;
    let node = found.node.unwrap_or_default();
    let observed = Observed {
        node,
        coordinate: found.name,
        branch: found.branch,
    };
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
