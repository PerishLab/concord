use super::super::super::emit;
use super::super::projection::Projection;
use concord_core::authority::Plumb;
use concord_core::{Coordinate, Error, Estate, Result, issue_delivery};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::path::Path;

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Refusal {
    code: String,
    message: String,
    details: Option<Value>,
}

pub struct Parameters<'a> {
    pub issue: &'a str,
    pub revision: i64,
    pub base: String,
    pub carry: bool,
    pub command: &'a Path,
    pub timeout: u64,
}

pub async fn prepare(estate: &Estate, parameters: Parameters<'_>, output: bool) -> Result<()> {
    let prepared = request(estate, &parameters).await;
    match prepared {
        Ok(plan) => emit(json!({"plan": plan}), output),
        Err(error) if parameters.carry => emit(json!({"error": Refusal::capture(&error)}), output),
        Err(error) => Err(error),
    }
}

async fn request(estate: &Estate, parameters: &Parameters<'_>) -> Result<issue_delivery::Plan> {
    let issue = Coordinate::parse(parameters.issue)?;
    let observed = Projection::new(parameters.command, 100, 1, parameters.timeout)
        .delivery(&issue)
        .await?;
    issue_delivery::prepare::<Plumb>(
        estate,
        &issue_delivery::Request {
            issue,
            revision: parameters.revision,
            base: parameters.base.clone(),
            snapshot: observed.snapshot,
            observed: observed.observed,
            outcome: observed.outcome,
        },
    )
    .await
}

impl Refusal {
    fn capture(error: &Error) -> Self {
        Self {
            code: error.code().to_string(),
            message: error.message().to_string(),
            details: error.details().cloned(),
        }
    }

    pub fn restore(self) -> Error {
        match self.details {
            Some(details) => Error::detailed(self.code, self.message, details),
            None => Error::typed(self.code, self.message),
        }
    }
}
