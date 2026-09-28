mod archive;
mod plan;
mod resource;
mod types;

#[cfg(test)]
pub use types::SCHEMA;
pub use types::{
    Archive, ArchiveTask, Capacity, Disposition, DispositionPlan, IssueDestination, Mapping,
    Observation, Preflight, ResourceDisposition, TaskDisposition,
};

use super::{Inventory, SOURCE};
use crate::{Error, Result, Seat};
use std::collections::BTreeSet;
use types::PREFLIGHT;

struct Input {
    inventory: Inventory,
    archive: Archive,
    available: u64,
}

pub(super) async fn read(
    seat: &Seat,
    plan: &DispositionPlan,
    observations: &[Observation],
) -> Result<Preflight> {
    let inventory = seat.transition().inventory().await?;
    if plan.schema != types::SCHEMA {
        return Err(Error::typed(
            "concord.transition.plan_schema",
            format!("unsupported disposition plan schema: {}", plan.schema),
        ));
    }
    if plan.inventory != inventory.fingerprint {
        return Err(Error::typed(
            "concord.transition.inventory_drift",
            "disposition plan does not name the exact source inventory",
        ));
    }
    let archive = archive::read(seat, &inventory).await?;
    let available = fs2::available_space(&seat.space)?;
    let preflight = validate(
        seat,
        plan,
        observations,
        Input {
            inventory,
            archive,
            available,
        },
    )?;
    let repeated = seat.transition().inventory().await?;
    if repeated.fingerprint != preflight.inventory {
        return Err(Error::typed(
            "concord.transition.source_drift",
            "transition source changed during disposition preflight",
        ));
    }
    Ok(preflight)
}

fn validate(
    seat: &Seat,
    plan: &DispositionPlan,
    observations: &[Observation],
    input: Input,
) -> Result<Preflight> {
    let checked = plan::check(plan, observations, &input.inventory)?;
    let members = resource::check(
        seat,
        &resource::Set::members(&plan.members, &input.inventory.members),
        &checked.tasks,
    )?;
    let artifacts = resource::check(
        seat,
        &resource::Set::artifacts(&plan.artifacts, &input.inventory.artifacts),
        &checked.tasks,
    )?;
    let targets = members
        .iter()
        .chain(&artifacts)
        .map(|mapping| mapping.target.clone())
        .collect::<Vec<_>>();
    if targets.iter().collect::<BTreeSet<_>>().len() != targets.len() {
        return Err(Error::typed(
            "concord.transition.target_collision",
            "multiple resources name the same target path",
        ));
    }
    let required = required(&input.inventory, &input.archive)?;
    capacity(required, input.available)?;
    Ok(Preflight {
        schema: PREFLIGHT.to_string(),
        source: SOURCE.to_string(),
        inventory: input.inventory.fingerprint,
        database: input.inventory.database,
        sudo: input.inventory.sudo,
        tasks: plan.tasks.clone(),
        issues: checked.issues,
        members,
        artifacts,
        archive: input.archive,
        capacity: Capacity {
            filesystem: seat.space.clone(),
            required,
            available: input.available,
        },
    })
}

fn required(inventory: &Inventory, archive: &Archive) -> Result<u64> {
    inventory
        .database
        .bytes
        .checked_mul(3)
        .and_then(|bytes| {
            inventory
                .sudo
                .bytes
                .checked_mul(2)
                .and_then(|sudo| bytes.checked_add(sudo))
        })
        .and_then(|bytes| bytes.checked_add(archive.bytes))
        .ok_or_else(|| Error::typed("concord.transition.capacity", "required space overflows"))
}

fn capacity(required: u64, available: u64) -> Result<()> {
    if available < required {
        return Err(Error::typed(
            "concord.transition.capacity",
            format!("transition requires {required} bytes, only {available} are available"),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod unit {
    #[test]
    fn capacity() {
        let error = super::capacity(10, 9).expect_err("capacity must refuse");
        assert_eq!(error.code(), "concord.transition.capacity");
    }
}
