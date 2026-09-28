use super::super::{Inventory, SOURCE, evidence, exact, settled};
use super::{Archive, ArchiveTask};
use crate::{Error, Estate, Life, Result, Seat};
use sha2::{Digest as _, Sha256};

pub async fn read(seat: &Seat, inventory: &Inventory) -> Result<Archive> {
    settled(&seat.database())?;
    let database = evidence(&seat.database())?;
    let sudo = evidence(&seat.sudo())?;
    exact("database", &inventory.database, &database)?;
    exact("sudo", &inventory.sudo, &sudo)?;
    let possession = std::fs::read_to_string(seat.sudo())?;
    let core = seat
        .bind(super::super::super::super::released, &possession)
        .await
        .map_err(|error| {
            Error::typed(
                "concord.transition.source",
                format!("estate is not the exact {SOURCE} source model: {error}"),
            )
        })?;
    let estate = Estate {
        core,
        space: seat.space.clone(),
        anchors: false,
        execution: false,
    };
    let mut tasks = Vec::new();
    for task in inventory
        .tasks
        .iter()
        .filter(|task| task.life == Life::Retired)
    {
        tasks.push(ArchiveTask {
            current: estate.current(&task.identity).await?,
            phases: estate.phases(&task.identity).await?,
        });
    }
    drop(estate);
    let body = serde_json::to_vec(&(
        super::types::ARCHIVE,
        SOURCE,
        &inventory.fingerprint,
        &inventory.database,
        &tasks,
    ))
    .map_err(|error| Error::typed("concord.transition.archive", error.to_string()))?;
    exact("database", &database, &evidence(&seat.database())?)?;
    exact("sudo", &sudo, &evidence(&seat.sudo())?)?;
    Ok(Archive {
        schema: super::types::ARCHIVE.to_string(),
        source: SOURCE.to_string(),
        inventory: inventory.fingerprint.clone(),
        database: inventory.database.clone(),
        tasks,
        bytes: body.len() as u64,
        digest: format!("{:x}", Sha256::digest(body)),
    })
}
