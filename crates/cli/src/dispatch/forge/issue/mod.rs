mod native;
use super::super::{emit, input};
use super::projection;
use crate::args::issue::{Command, Preflight};
use concord_core::{Admission, Anchor, Coordinate, Error, Estate, Reconcile, Result};
use serde::{Deserialize, Serialize};
use serde_json::json;

pub(super) mod comment;
mod observe;
mod transfer;

const SCHEMA: &str = "concord.issue-delivery/v1";
const SHAPE: &str = r#"{"schema":"concord.issue-delivery/v1","anchor":{"key":1,"node":"I_node","owner":"PerishLab","repository":"concord","number":25,"revision":0},"observation":{"node":"I_node","stable":"R_node","owner":"PerishLab","repository":"concord","number":25,"url":"https://github.com/PerishLab/concord/issues/25","state":"open","kind":"Feature","updated":"2026-09-27T00:00:00Z","seen":0}}"#;

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Plan {
    schema: String,
    anchor: Anchor,
    observation: observe::Observed,
}

pub async fn run(
    estate: &Estate,
    command: Command,
    output: bool,
    phase: Option<&crate::phase::Binding>,
) -> Result<()> {
    match command {
        Command::Close { .. } => unreachable!("handled before estate open"),
        Command::Phase { .. } => unreachable!("handled before estate open"),
        Command::Bootstrap => unreachable!("handled before estate open"),
        Command::Brief {
            issue,
            size,
            children,
            blockers,
            blocking,
            pulls,
            timeout,
        } => {
            let coordinate = Coordinate::parse(&issue)?;
            let brief = projection::Projection::new(size, 1, timeout)
                .brief(
                    &coordinate,
                    projection::Request {
                        size,
                        children,
                        blockers,
                        blocking,
                        pulls,
                        comments: None,
                    },
                )
                .await?;
            let observed = crate::phase::observe(phase, &issue, &brief.issue.node, timeout).await;
            let mut value = json!(brief);
            value["phase"] = observed;
            emit(json!({"brief": value}), output)
        }
        Command::Preflight { command } => preflight(estate, command, output, phase).await,
        Command::Graph {
            issue,
            size,
            nodes,
            pages,
            timeout,
        } => {
            let coordinate = Coordinate::parse(&issue)?;
            let graph = projection::Projection::new(size, pages, timeout)
                .graph(&coordinate, nodes)
                .await;
            emit(json!({"graph": graph}), output)
        }
        Command::Ready(relations) => {
            let coordinate = Coordinate::parse(&relations.issue)?;
            let readiness =
                projection::Projection::new(relations.size, relations.pages, relations.timeout)
                    .ready(&coordinate)
                    .await?;
            emit(json!({"readiness": readiness}), output)
        }
        Command::Comment {
            issue,
            input,
            timeout,
        } => {
            let coordinate = Coordinate::parse(&issue)?;
            let comment = comment::declare(&coordinate, &input, timeout).await?;
            comment::output(comment, output)
        }
        Command::Delivery { .. } => unreachable!("delivery completion is handled by dispatch"),
        Command::Attach { issue, timeout } => {
            let coordinate = Coordinate::parse(&issue)?;
            let observed = observe::issue(&coordinate, timeout).await?;
            concord_core::automation::admit(&observed.kind)?;
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
            timeout,
        } => {
            let coordinate = Coordinate::parse(&issue)?;
            let anchor = estate.issue(&coordinate).await?;
            stale(anchor.revision, revision)?;
            let observation = observe::issue(&coordinate, timeout).await?;
            node(&anchor, &observation)?;
            emit(
                json!({"plan": Plan { schema: SCHEMA.to_string(), anchor, observation }}),
                output,
            )
        }
        Command::Validate {
            input: path,
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
            let observation = observe::issue(&anchor.coordinate, timeout).await?;
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
        Command::Transfer(request) => transfer::run(estate, &request, output).await,
        Command::Reconcile {
            issue,
            coordinate,
            revision,
            timeout,
        } => {
            let anchor = Coordinate::parse(&issue)?;
            let coordinate = Coordinate::parse(&coordinate)?;
            let observed = observe::issue(&coordinate, timeout).await?;
            concord_core::automation::admit(&observed.kind)?;
            let anchor = estate
                .reconcile(&Reconcile {
                    anchor,
                    node: observed.node,
                    repository: observed.stable,
                    coordinate,
                    revision,
                })
                .await?;
            emit(json!({"anchor": anchor}), output)
        }
    }
}

async fn preflight(
    estate: &Estate,
    command: Preflight,
    output: bool,
    phase: Option<&crate::phase::Binding>,
) -> Result<()> {
    match command {
        Preflight::Proposed {
            repository,
            kind,
            title,
            outcome,
            size,
            pages,
            timeout,
        } => {
            let report = super::preflight::run(
                estate,
                super::preflight::Request {
                    repository: &repository,
                    kind: &kind,
                    title: &title,
                    outcome: &outcome,
                    bounds: super::preflight::Bounds {
                        first: size,
                        pages,
                        timeout,
                    },
                },
            )
            .await?;
            emit(json!({"preflight": report}), output)
        }
        Preflight::Existing(relations) => {
            let report = super::preflight::existing(estate, &relations).await?;
            let observed = crate::phase::observe(
                phase,
                &relations.issue,
                &report.issue.node,
                relations.timeout,
            )
            .await;
            let mut value = json!(report);
            value["phase"] = observed;
            emit(json!({"preflight": value}), output)
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
        left.stable == right.stable,
        left.coordinate == right.coordinate,
        left.url == right.url,
        left.state == right.state,
        left.kind == right.kind,
        left.updated == right.updated,
    ]
    .into_iter()
    .all(|held| held)
}
