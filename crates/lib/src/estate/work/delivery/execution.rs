use super::Plan;
use crate::{Error, Estate, Result};
use radicula_lock::{Factory, Policy, Wait};
pub use radicula_lock::{Failure, file::Permit};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs::{File, OpenOptions};
use std::io::Read;
use std::path::Path;

#[derive(Deserialize, Serialize)]
struct Holder {
    schema: String,
    node: String,
    pid: u32,
    plan: String,
}

pub async fn acquire(estate: &Estate, plan: &Plan) -> Result<Permit> {
    let anchor = {
        let _guard = estate.share()?;
        estate.issue(&plan.issue).await?
    };
    if anchor.node != plan.node {
        return Err(Error::typed(
            "concord.delivery.coordinate",
            "delivery plan and current Anchor have different stable Issue identities",
        ));
    }
    let holder = Holder {
        schema: "concord.delivery-holder/v1".into(),
        node: anchor.node,
        pid: std::process::id(),
        plan: format!(
            "{:x}",
            Sha256::digest(
                serde_json::to_vec(plan).map_err(|error| Error::new(error.to_string()))?
            )
        ),
    };
    let path = estate
        .space
        .join(".concord")
        .join(format!("land-{}.lock", anchor.key));
    let file = open(&path)?;
    let gate = Factory::file(file, Policy::EXCLUSIVE).map_err(refused)?;
    let permit = gate.acquire(Wait::immediate()).map_err(|error| {
        if matches!(error, radicula_lock::Error::Busy) {
            busy(&path.with_extension("json"), plan)
        } else {
            refused(error)
        }
    })?;
    if let Err(error) = record(&path.with_extension("json"), &holder) {
        return match permit.release() {
            Ok(()) => Err(error),
            Err(release) => Err(Error::detailed(
                "concord.delivery.execution",
                format!("holder publication failed and release is unconfirmed: {release}"),
                serde_json::json!({"publication": error.message()}),
            )),
        };
    }
    Ok(permit)
}

fn record(path: &Path, holder: &Holder) -> Result<()> {
    if std::fs::symlink_metadata(path)
        .is_ok_and(|metadata| !metadata.is_file() || metadata.file_type().is_symlink())
    {
        return Err(Error::typed(
            "concord.delivery.execution",
            "holder path is not a direct regular file; preserve and inspect it",
        ));
    }
    crate::path::at(path)
        .file(&serde_json::to_string(holder).map_err(|error| Error::new(error.to_string()))?)
}

fn open(path: &Path) -> Result<File> {
    if let Ok(metadata) = std::fs::symlink_metadata(path)
        && (!metadata.is_file() || metadata.file_type().is_symlink())
    {
        return Err(Error::typed(
            "concord.delivery.execution",
            "land lock path is not a direct regular file; preserve and inspect it",
        ));
    }
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)?;
    crate::path::at(path).mode(0o600)?;
    Ok(file)
}

fn busy(path: &Path, plan: &Plan) -> Error {
    let holder = File::open(path)
        .ok()
        .and_then(|file| serde_json::from_reader::<_, Holder>(file.take(4096)).ok())
        .filter(|holder| holder.schema == "concord.delivery-holder/v1" && holder.node == plan.node);
    Error::detailed(
        "concord.delivery.running",
        format!(
            "another land invocation holds {}/{}#{}; inspect the holder observation and wait for it to exit before resuming the exact plan",
            plan.issue.owner, plan.issue.repository, plan.issue.number
        ),
        serde_json::json!({"issue": plan.issue, "node": plan.node, "holder": holder, "semantics": "kernel admission owns exclusion; holder fields are observations, not authority; absent or partial diagnostics remain unknown"}),
    )
}

fn refused(error: radicula_lock::Error) -> Error {
    Error::typed(
        "concord.delivery.execution",
        format!("land execution admission refused: {error}"),
    )
}
