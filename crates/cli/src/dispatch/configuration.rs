use crate::args::Configure;
use crate::args::migration::Command;
use crate::args::transition::Command as TransitionCommand;
use crate::config::Config;
use crate::output;
use concord_core::migration::{Plan as MigrationPlan, Receipt as MigrationReceipt};
use concord_core::{Result, Seat};
use serde::Deserialize;
use serde_json::json;

use super::{emit, explicit, input};

pub fn run(config: &Config, command: Configure, json_output: bool) -> Result<()> {
    match command {
        Configure::Path => unreachable!("config path exits before loading the space"),
        Configure::Show => show(config, json_output),
    }
}

fn show(config: &Config, json_output: bool) -> Result<()> {
    output::value(
        json!({
            "domain_space_root": config.domain_space_root.display().to_string(),
            "home": config.home.display().to_string(),
            "releases": config.releases,
            "depot": config.depot,
        }),
        json_output,
    );
    Ok(())
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Plan {
    version: u64,
    plan: MigrationPlan,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Receipt {
    version: u64,
    receipt: MigrationReceipt,
}

pub(crate) async fn migration(seat: &Seat, command: Command, output: bool) -> Result<()> {
    let value = match command {
        Command::Preflight => json!({"survey": seat.migration().survey().await?}),
        Command::Prepare { fingerprint } => {
            json!({"plan": seat.migration().prepare(&fingerprint).await?})
        }
        Command::Apply { plan, apply } => {
            explicit(apply, "migration apply")?;
            let envelope: Plan = input::read(&plan, PLAN)?;
            version(envelope.version)?;
            json!({"receipt": seat.migration().apply(&envelope.plan).await?})
        }
        Command::Rollback { receipt, apply } => {
            explicit(apply, "migration rollback")?;
            let envelope: Receipt = input::read(&receipt, RECEIPT)?;
            version(envelope.version)?;
            json!({"rollback": seat.migration().rollback(&envelope.receipt).await?})
        }
    };
    emit(value, output)
}

pub(crate) async fn transition(
    seat: &Seat,
    command: TransitionCommand,
    output: bool,
) -> Result<()> {
    let value = match command {
        TransitionCommand::Inventory => json!({"inventory": seat.transition().inventory().await?}),
        TransitionCommand::Preflight {
            plan,
            command,
            timeout,
        } => {
            let plan: concord_core::TransitionDispositionPlan = input::read(&plan, DISPOSITION)?;
            let destinations = destinations(&plan);
            let mut observations = Vec::with_capacity(destinations.len());
            for coordinate in destinations.into_values() {
                let observed = super::forge::observe::issue(&coordinate, &command, timeout).await?;
                observations.push(concord_core::TransitionObservation {
                    node: observed.node,
                    coordinate,
                });
            }
            json!({"preflight": seat.transition().preflight(&plan, &observations).await?})
        }
    };
    emit(value, output)
}

fn destinations(
    plan: &concord_core::TransitionDispositionPlan,
) -> std::collections::BTreeMap<String, concord_core::Coordinate> {
    plan.tasks
        .iter()
        .filter_map(|task| match &task.disposition {
            concord_core::transition::Disposition::Issue { issue } => Some(issue),
            concord_core::transition::Disposition::ArchiveOnly => None,
        })
        .map(|issue| (issue.coordinate.identity(), issue.coordinate.clone()))
        .collect()
}

const DISPOSITION: &str = r#"{
  "schema": "concord.v0.13-disposition-plan/v1",
  "inventory": "SHA256",
  "tasks": [{"task":"DOMAIN/NAME","revision":0,"disposition":{"kind":"issue","node":"I_NODE","owner":"OWNER","repository":"REPOSITORY","number":1}}],
  "members": [],
  "artifacts": []
}"#;

fn version(version: u64) -> Result<()> {
    if version == 1 {
        return Ok(());
    }
    Err(concord_core::Error::typed(
        "concord.migration.envelope",
        "migration envelope version must be 1",
    ))
}

const PLAN: &str = r#"{
  "version": 1,
  "plan": {"schema": "concord.estate-migration/v1"}
}"#;

const RECEIPT: &str = r#"{
  "version": 1,
  "receipt": {"schema": "concord.estate-migration-receipt/v1"}
}"#;
