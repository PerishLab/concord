mod flow;
mod storage;

#[cfg(test)]
mod tests;

use super::super::Seat;
use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub const SCHEMA: &str = "concord.estate-migration/v1";
pub const RECEIPT: &str = "concord.estate-migration-receipt/v1";
pub const SOURCE: &str = "v0.12.10";
pub const TARGET: &str = "v0.13.0";

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Evidence {
    pub path: PathBuf,
    pub bytes: u64,
    pub digest: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Survey {
    pub schema: String,
    pub source: String,
    pub target: String,
    pub fingerprint: String,
    pub database: Evidence,
    pub sudo: Evidence,
    pub stage: PathBuf,
    pub backup: PathBuf,
    pub required: u64,
    pub available: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Plan {
    pub schema: String,
    pub survey: Survey,
    pub staged: Evidence,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Receipt {
    pub schema: String,
    pub source: Survey,
    pub database: Evidence,
    pub backup: Evidence,
    pub record: PathBuf,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Rollback {
    pub schema: String,
    pub source: String,
    pub restored: Evidence,
}

pub struct Migration<'a> {
    seat: &'a Seat,
}

impl Seat {
    pub fn migration(&self) -> Migration<'_> {
        Migration { seat: self }
    }
}

impl Migration<'_> {
    fn receipt(&self, receipt: &Receipt) -> Result<()> {
        if !coordinates(receipt) {
            return Err(Error::typed(
                "concord.migration.receipt",
                "migration receipt coordinates do not match this binary",
            ));
        }
        exact(
            "source database path",
            &self.seat.database(),
            &receipt.source.database.path,
        )?;
        exact(
            "source sudo path",
            &self.seat.sudo(),
            &receipt.source.sudo.path,
        )?;
        exact(
            "activated database path",
            &self.seat.database(),
            &receipt.database.path,
        )?;
        exact(
            "source fingerprint",
            &receipt.source.fingerprint,
            &storage::fingerprint(&receipt.source.database, &receipt.source.sudo),
        )?;
        let root = self
            .seat
            .root
            .join("migration")
            .join(TARGET)
            .join(&receipt.source.fingerprint);
        exact("stage path", &root.join("stage"), &receipt.source.stage)?;
        exact("backup path", &root.join("backup"), &receipt.source.backup)?;
        exact(
            "backup database path",
            &root.join("backup/estate.sqlite3"),
            &receipt.backup.path,
        )?;
        exact("receipt path", &root.join("receipt.json"), &receipt.record)
    }
}

fn coordinates(receipt: &Receipt) -> bool {
    if receipt.schema != RECEIPT || receipt.source.schema != SCHEMA {
        return false;
    }
    if receipt.source.source != SOURCE || receipt.source.target != TARGET {
        return false;
    }
    true
}

fn validate(plan: &Plan) -> Result<()> {
    if plan.schema != SCHEMA || plan.survey.schema != SCHEMA {
        return Err(Error::typed(
            "concord.migration.plan",
            "unsupported migration plan schema",
        ));
    }
    if plan.survey.source != SOURCE || plan.survey.target != TARGET {
        return Err(Error::typed(
            "concord.migration.plan",
            "migration plan release coordinates do not match this binary",
        ));
    }
    Ok(())
}

fn exact<T: Eq + std::fmt::Debug + ?Sized>(label: &str, expected: &T, found: &T) -> Result<()> {
    if expected == found {
        return Ok(());
    }
    Err(Error::typed(
        "concord.migration.drift",
        format!("{label} changed: expected {expected:?}, found {found:?}"),
    ))
}

fn align() {
    let tick = keel::life::tick();
    while keel::life::tick() == tick {
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
}
