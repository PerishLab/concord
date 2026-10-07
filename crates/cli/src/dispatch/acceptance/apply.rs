use concord_core::acceptance::{Step, Target};
use concord_core::{Coordinate, Error, Result};
use serde_json::{Value, json};

use super::super::forge;
use super::model::Snapshot;
use super::plan::Envelope;
use crate::args::acceptance::{Apply, Observe};

pub(super) async fn run(args: Apply) -> Result<Value> {
    if !args.apply {
        return Err(super::fault(
            "consent",
            "acceptance writes require explicit --apply",
        ));
    }
    let plan: Envelope =
        crate::dispatch::input::read(&args.prepare.input, "prepared acceptance plan")?;
    let args = args.prepare.observe;
    plan.validate(&Coordinate::parse(&args.issue)?)?;
    let execution = forge::execution()?;
    let mut snapshot = super::observe(&args).await?;
    plan.recover(&snapshot)?;
    let writer = Writer {
        args: &args,
        plan: &plan,
    };
    let result = writer.execute(&mut snapshot).await;
    match result {
        Ok(()) => Ok(json!({"acceptance": {
            "recovery": plan.recover(&snapshot)?, "snapshot": snapshot,
            "plan": plan, "execution": execution,
            "semantics": "observed complete attestation; no readiness proof, atomic transaction or Issue closing"
        }})),
        Err(error) => Err(super::fault("partial", json!({
            "failure": error.to_string(), "snapshot": snapshot, "plan": plan,
            "semantics": "last confirmed observation only; completion may be unknown; retry the same plan"
        }).to_string())),
    }
}

struct Writer<'a> {
    args: &'a Observe,
    plan: &'a Envelope,
}

impl Writer<'_> {
    async fn execute(&self, snapshot: &mut Snapshot) -> Result<()> {
        let args = self.args;
        let plan = self.plan;
        if plan.recover(snapshot)?.step == Step::Publish {
            self.publish(snapshot).await?;
        }
        let recovery = plan.recover(snapshot)?;
        if recovery.step == Step::Complete {
            return Ok(());
        }
        let target =
            super::labels::resolve(args, &snapshot.header.repository, recovery.target, true)
                .await?;
        self.refresh(snapshot).await?;
        if plan.recover(snapshot)?.step == Step::Complete {
            return Ok(());
        }
        if let Some(prior) = Target::select(&snapshot.labels)? {
            let label =
                super::labels::resolve(args, &snapshot.header.repository, prior, false).await?;
            self.refresh(snapshot).await?;
            if plan.recover(snapshot)?.step == Step::Complete {
                return Ok(());
            }
            if Target::select(&snapshot.labels)? == Some(prior) {
                let result = super::labels::alter(args, &snapshot.header.node, &label, true).await;
                self.confirm(snapshot, result).await?;
                if Target::select(&snapshot.labels)?.is_some() {
                    return Err(super::fault(
                        "disagreement",
                        "managed predecessor label remains",
                    ));
                }
            }
        }
        if plan.recover(snapshot)?.step != Step::Complete {
            let result = super::labels::alter(args, &snapshot.header.node, &target, false).await;
            self.confirm(snapshot, result).await?;
        }
        if plan.recover(snapshot)?.step != Step::Complete {
            return Err(super::fault(
                "disagreement",
                "managed target update is not observed complete",
            ));
        }
        Ok(())
    }

    async fn publish(&self, snapshot: &mut Snapshot) -> Result<()> {
        let args = self.args;
        let plan = self.plan;
        let body = plan.plan.intent.body()?;
        let result = forge::submit(&args.command, &snapshot.header.node, body, args.timeout).await;
        let acknowledgment =
            result.and_then(|reply| forge::verify(&snapshot.header.coordinate, body, &reply));
        self.refresh(snapshot).await?;
        if plan.recover(snapshot)?.step == Step::Publish {
            return Err(acknowledgment.err().unwrap_or_else(|| {
                super::fault(
                    "disagreement",
                    "acknowledged comment is absent from complete observation",
                )
            }));
        }
        Ok(())
    }

    async fn refresh(&self, snapshot: &mut Snapshot) -> Result<()> {
        let current = super::observe(self.args).await?;
        *snapshot = current;
        self.plan.recover(snapshot)?;
        Ok(())
    }

    async fn confirm(&self, snapshot: &mut Snapshot, result: Result<()>) -> Result<()> {
        let observed = self.refresh(snapshot).await;
        match (result, observed) {
            (Ok(()), observed) => observed,
            (Err(error), Ok(())) => Err(error),
            (Err(error), Err(observed)) => Err(Error::typed(
                "concord.acceptance.indeterminate",
                format!("mutation: {error}; observation: {observed}"),
            )),
        }
    }
}
