use super::super::{emit, forge::provider};
use crate::args::Observe;
use concord_core::{Coordinate, Estate, Result};
use serde_json::json;

pub(super) async fn run(
    estate: &Estate,
    coordinate: (&str, &str),
    observation: Observe,
    output: bool,
) -> Result<()> {
    if super::issue(coordinate.0) {
        anchor(estate, coordinate, observation, output).await
    } else {
        task(estate, coordinate, observation, output).await
    }
}

async fn task(
    estate: &Estate,
    coordinate: (&str, &str),
    observation: Observe,
    output: bool,
) -> Result<()> {
    let status = estate.member_status(coordinate.0, coordinate.1).await?;
    let observed = if observation.observe {
        Some(
            provider::observe(
                provider::Projection::Member,
                status.reference.as_ref(),
                observation.command.as_deref(),
                observation.timeout,
            )
            .await,
        )
    } else {
        None
    };
    if output {
        return emit(json!({"status": status, "observation": observed}), true);
    }
    super::super::output::member_status(&status);
    if let Some(observed) = &observed {
        provider::print(observed);
    }
    Ok(())
}

async fn anchor(
    estate: &Estate,
    coordinate: (&str, &str),
    observation: Observe,
    output: bool,
) -> Result<()> {
    let issue = Coordinate::parse(coordinate.0)?;
    let status = estate.issue_member_status(&issue, coordinate.1).await?;
    let mut observed = Vec::new();
    if observation.observe {
        for reference in &status.references {
            observed.push(
                provider::observe(
                    provider::Projection::Member,
                    Some(reference),
                    observation.command.as_deref(),
                    observation.timeout,
                )
                .await,
            );
        }
    }
    if output {
        return emit(json!({"status": status, "observations": observed}), true);
    }
    super::super::output::issue_member_status(&status);
    for observation in &observed {
        provider::print(observation);
    }
    Ok(())
}
