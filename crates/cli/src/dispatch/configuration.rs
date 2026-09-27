use crate::args::Configure;
use crate::args::migration::Command;
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
