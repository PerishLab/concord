use concord_core::Result;
use concord_core::acceptance::{Check, Evaluation, Fact, History, Observation, Verdict};
use serde_json::{Value, json};

use crate::args::acceptance::Observe;
use serde::Serialize;

mod closing;
mod pull;
mod release;

pub(super) use closing::run as closing;

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub(in crate::dispatch) struct Projection {
    pub(super) snapshot: super::model::Snapshot,
    pub evaluation: Evaluation,
}

impl Projection {
    pub(in crate::dispatch) fn report(&self) -> Result<concord_core::acceptance::Report> {
        let history = History::read(&self.snapshot.comments())?;
        let promise = match history.declaration.map(|record| record.marker) {
            Some(concord_core::acceptance::Marker::Declaration { promise, .. })
            | Some(concord_core::acceptance::Marker::Amendment { promise, .. }) => promise,
            None if self.evaluation.target.is_none() => "No declared target; the existing detailed-condition closure flow remains applicable.".into(),
            _ => {
                return Err(super::fault(
                    "missing",
                    "adopted delivery has no current declaration",
                ));
            }
        };
        Ok(concord_core::acceptance::Report {
            evaluation: self.evaluation.clone(),
            promise,
            checklist: concord_core::acceptance::sections(&self.snapshot.header.body)
                .remove("acceptance")
                .unwrap_or_default(),
        })
    }

    pub(in crate::dispatch) fn current(&self, node: &str, kind: &str, body: &str) -> bool {
        self.snapshot.header.node == node
            && self.snapshot.header.kind == kind
            && self.snapshot.header.body == body
    }

    pub(in crate::dispatch) fn accepted(&self) -> bool {
        if self.evaluation.target.is_some() {
            return self.evaluation.verdict == Verdict::Satisfied;
        }
        let names = ["sections", "checklist", "children", "blockers"];
        names.iter().all(|name| self.settled(name))
    }

    pub(in crate::dispatch) fn settled(&self, name: &str) -> bool {
        self.evaluation
            .checks
            .iter()
            .any(|check| check.name == name && check.fact.verdict == Verdict::Satisfied)
    }
}

pub(super) async fn run(args: Observe) -> Result<Value> {
    Ok(json!({"acceptance": read(&args).await?}))
}

pub(in crate::dispatch) async fn read(args: &Observe) -> Result<Projection> {
    let snapshot = super::observe(args).await?;
    let comments = snapshot.comments();
    let history = History::read(&comments)?;
    history.target(&snapshot.labels)?;
    let reader = super::read::Reader {
        coordinate: &snapshot.header.coordinate,
        command: &args.command,
        timeout: args.timeout,
        pages: args.pages,
    };
    let first = reader.relations(&snapshot.header).await?;
    let relations = reader.relations(&snapshot.header).await?;
    if first != relations {
        return Err(super::fault(
            "changed",
            "native acceptance relationships changed during evaluation",
        ));
    }
    let (checks, reasons) = super::super::forge::conditions(
        &snapshot.header.body,
        &snapshot.header.kind,
        relations.children(),
        relations.blockers(),
    );
    let conditions = [
        ("sections", checks.required_sections),
        ("checklist", checks.acceptance_settled),
        ("children", checks.sub_issues_closed),
        ("blockers", checks.blockers_closed),
    ]
    .into_iter()
    .map(|(name, satisfied)| Check {
        name: name.into(),
        fact: Fact {
            verdict: if satisfied {
                Verdict::Satisfied
            } else {
                Verdict::Unmet
            },
            reason: if satisfied {
                format!("current {name} condition is settled")
            } else {
                reasons.join("; ")
            },
        },
    })
    .collect();
    let freshness = match (&history.declaration, &history.closure) {
        (Some(declaration), Some(closure)) => {
            let reader = super::freshness::Reader {
                args,
                snapshot: &snapshot,
                declaration: &declaration.reference,
                closure: &closure.reference,
            };
            let first = reader.complete().await?;
            let second = reader.complete().await?;
            if first != second || super::observe(args).await? != snapshot {
                return Err(super::fault(
                    "changed",
                    "acceptance facts changed during evaluation",
                ));
            }
            second.fact()?
        }
        _ => Fact {
            verdict: Verdict::Unknown,
            reason: "no current review to bind freshness".into(),
        },
    };
    if reader.relations(&snapshot.header).await? != relations {
        return Err(super::fault(
            "changed",
            "native acceptance conditions moved during review observation",
        ));
    }
    let release = release::read(args, &snapshot.header, &history).await?;
    if history
        .declaration
        .as_ref()
        .is_some_and(|record| record.marker.target() == concord_core::acceptance::Target::Release)
        && super::observe(args).await? != snapshot
    {
        return Err(super::fault(
            "changed",
            "acceptance changed during release observation",
        ));
    }
    let evaluation = Evaluation::read(Observation {
        labels: &snapshot.labels,
        comments: &comments,
        body: &snapshot.header.body,
        freshness,
        conditions,
        release,
    })?;
    Ok(Projection {
        snapshot,
        evaluation,
    })
}
