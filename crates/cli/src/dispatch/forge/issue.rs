use super::super::{emit, input};
use super::{observe, projection};
use crate::args::issue::Command;
use concord_core::{Admission, Anchor, Coordinate, Error, Estate, Reconcile, Result};
use serde::{Deserialize, Serialize};
use serde_json::json;

const SCHEMA: &str = "concord.issue-delivery/v1";
const SHAPE: &str = r#"{"schema":"concord.issue-delivery/v1","anchor":{"key":1,"node":"I_node","owner":"PerishLab","repository":"concord","number":25,"revision":0},"observation":{"node":"I_node","owner":"PerishLab","repository":"concord","number":25,"url":"https://github.com/PerishLab/concord/issues/25","state":"open","kind":"Feature","updated":"2026-09-27T00:00:00Z","seen":0}}"#;

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Plan {
    schema: String,
    anchor: Anchor,
    observation: observe::Observed,
}

pub async fn run(estate: &Estate, command: Command, output: bool) -> Result<()> {
    match command {
        Command::Brief {
            issue,
            command,
            page_size,
            sub_issues_after,
            blocked_by_after,
            blocking_after,
            pulls_after,
            timeout,
        } => {
            let coordinate = Coordinate::parse(&issue)?;
            let brief = projection::Projection::new(&command, page_size, 1, timeout)
                .brief(
                    &coordinate,
                    projection::PageRequest {
                        size: page_size,
                        sub_issues_after,
                        blocked_by_after,
                        blocking_after,
                        pulls_after,
                        comments_after: None,
                    },
                )
                .await?;
            emit(json!({"brief": brief}), output)
        }
        Command::Graph {
            issue,
            command,
            page_size,
            max_nodes,
            max_pages,
            timeout,
        } => {
            let coordinate = Coordinate::parse(&issue)?;
            let graph = projection::Projection::new(&command, page_size, max_pages, timeout)
                .graph(&coordinate, max_nodes)
                .await;
            emit(json!({"graph": graph}), output)
        }
        Command::Ready {
            issue,
            command,
            page_size,
            max_pages,
            timeout,
        } => {
            let coordinate = Coordinate::parse(&issue)?;
            let readiness = projection::Projection::new(&command, page_size, max_pages, timeout)
                .ready(&coordinate)
                .await?;
            emit(json!({"readiness": readiness}), output)
        }
        Command::Attach {
            issue,
            command,
            timeout,
        } => {
            let coordinate = Coordinate::parse(&issue)?;
            let observed = observe::issue(&coordinate, &command, timeout).await?;
            let anchor = estate
                .admit(&Admission {
                    node: observed.node,
                    coordinate,
                })
                .await?;
            emit(json!({"anchor": anchor}), output)
        }
        Command::Show { issue } => {
            let coordinate = Coordinate::parse(&issue)?;
            emit(json!({"anchor": estate.issue(&coordinate).await?}), output)
        }
        Command::Prepare {
            issue,
            revision,
            command,
            timeout,
        } => {
            let coordinate = Coordinate::parse(&issue)?;
            let anchor = estate.issue(&coordinate).await?;
            stale(anchor.revision, revision)?;
            let observation = observe::issue(&coordinate, &command, timeout).await?;
            node(&anchor, &observation)?;
            emit(
                json!({"plan": Plan { schema: SCHEMA.to_string(), anchor, observation }}),
                output,
            )
        }
        Command::Validate {
            input: path,
            command,
            timeout,
        } => {
            let plan: Plan = input::read(&path, SHAPE)?;
            if plan.schema != SCHEMA {
                return Err(Error::typed(
                    "concord.issue.plan",
                    "unsupported Issue delivery plan schema",
                ));
            }
            let anchor = estate.issue(&plan.anchor.coordinate).await?;
            if anchor != plan.anchor {
                return Err(Error::typed(
                    "concord.issue.anchor_drift",
                    "local Issue execution anchor changed after delivery preparation",
                ));
            }
            node(&anchor, &plan.observation)?;
            let observation = observe::issue(&anchor.coordinate, &command, timeout).await?;
            if !same(&observation, &plan.observation) {
                return Err(Error::typed(
                    "concord.issue.observation_drift",
                    "GitHub Issue observation changed after delivery preparation",
                ));
            }
            emit(
                json!({"anchor": anchor, "observation": observation}),
                output,
            )
        }
        Command::Reconcile {
            issue,
            coordinate,
            revision,
            command,
            timeout,
        } => {
            let anchor = Coordinate::parse(&issue)?;
            let coordinate = Coordinate::parse(&coordinate)?;
            let observed = observe::issue(&coordinate, &command, timeout).await?;
            let anchor = estate
                .reconcile(&Reconcile {
                    anchor,
                    node: observed.node,
                    coordinate,
                    revision,
                })
                .await?;
            emit(json!({"anchor": anchor}), output)
        }
    }
}

fn stale(found: i64, expected: i64) -> Result<()> {
    if found == expected {
        return Ok(());
    }
    Err(Error::typed(
        "concord.issue.stale",
        format!("Issue execution revision changed: expected {expected}, found {found}"),
    ))
}

fn node(anchor: &Anchor, observation: &observe::Observed) -> Result<()> {
    if anchor.node == observation.node {
        return Ok(());
    }
    Err(Error::typed(
        "concord.issue.node_mismatch",
        format!(
            "Issue node changed: expected {}, found {}",
            anchor.node, observation.node
        ),
    ))
}

fn same(left: &observe::Observed, right: &observe::Observed) -> bool {
    [
        left.node == right.node,
        left.coordinate == right.coordinate,
        left.url == right.url,
        left.state == right.state,
        left.kind == right.kind,
        left.updated == right.updated,
    ]
    .into_iter()
    .all(|held| held)
}
