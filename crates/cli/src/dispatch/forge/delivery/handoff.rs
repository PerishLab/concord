use super::super::super::emit;
use super::adaptor;
use super::branches::Remote;
use super::git::Git;
use concord_core::authority::Plumb;
use concord_core::{Coordinate, Error, Estate, Integration, Result, delivery};
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
    pub authority: delivery::Mode,
    pub review: Option<&'a Path>,
    pub issue: &'a str,
    pub revision: i64,
    pub base: String,
    pub carry: bool,
    pub timeout: u64,
}

pub async fn prepare(estate: &Estate, parameters: Parameters<'_>, output: bool) -> Result<()> {
    let prepared = request(estate, &parameters).await;
    match prepared {
        Ok(plan) => {
            crate::output::value(
                json!({"version": 2, "forge": adaptor::Identity::of(&plan), "plan": plan}),
                output,
            );
            Ok(())
        }
        Err(error) if parameters.carry => emit(json!({"error": Refusal::capture(&error)}), output),
        Err(error) => Err(error),
    }
}

async fn request(estate: &Estate, parameters: &Parameters<'_>) -> Result<delivery::Plan> {
    let review = parameters
        .review
        .map(|path| crate::dispatch::input::read(path, delivery::Review::SHAPE))
        .transpose()?;
    if (parameters.authority == delivery::Mode::Manual) != review.is_some() {
        return Err(Error::typed(
            "concord.delivery.review",
            "governance-manual requires --review; other authorities refuse review input",
        ));
    }
    let issue = Coordinate::parse(parameters.issue)?;
    let member = estate.member(&issue).await?;
    adaptor::integration(&member.integration).await?;
    let observed = adaptor::observe(parameters.timeout, &issue).await?;
    let remote = super::branches::clean(estate, &member.integration, parameters.timeout).await?;
    drift(&member.integration, &parameters.base, &remote).await?;
    delivery::prepare::<Plumb>(
        estate,
        &delivery::Request {
            authority: parameters.authority,
            review,
            issue,
            revision: parameters.revision,
            base: parameters.base.clone(),
            snapshot: observed.snapshot,
            observed: observed.observed,
            outcome: observed.outcome,
            acceptance: observed.acceptance,
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

async fn drift(integration: &Integration, base: &str, remote: &Remote) -> Result<()> {
    let Some(upstream) = remote.heads.get(base) else {
        return Ok(());
    };
    let tracking = format!("refs/remotes/{}/{base}", integration.remote);
    let local = Git::optional(Path::new(&integration.path), &tracking).await?;
    if local.is_empty() || local == *upstream {
        return Ok(());
    }
    let notice = if remote.auto.is_empty() {
        String::new()
    } else {
        format!(
            "; active Auto follow branch(es) on {}: {}",
            integration.remote,
            remote.auto.join(", ")
        )
    };
    let kind = &integration.remote;
    Err(Error::detailed(
        "concord.delivery.stale",
        format!(
            "remote {kind}/{base} ({upstream}) differs from local {kind}/{base} ({local}); fetch {kind}, merge {kind}/{base} into the Member, and rerun plumb guard before preparing delivery{notice}"
        ),
        json!({
            "base": base,
            "local": local,
            "remote": upstream,
            "auto": remote.auto,
        }),
    ))
}
