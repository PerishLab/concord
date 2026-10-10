use super::{SHAPE, branches, client, handoff::Refusal, projection, pull, settle, version};
use crate::args::issue::Delivery;
use crate::dispatch::{emit, input};
use concord_core::authority::Plumb;
use concord_core::issue_delivery::Check;
use concord_core::{Coordinate, Error, Estate, IssueDeclaration, Result, issue_delivery};
use serde::Deserialize;
use serde_json::json;
use std::path::{Path, PathBuf};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Envelope {
    version: u64,
    #[serde(default)]
    plan: Option<issue_delivery::Plan>,
    #[serde(default)]
    error: Option<Refusal>,
}

struct Flow<'a> {
    estate: &'a Estate,
    plan: issue_delivery::Plan,
    command: PathBuf,
    timeout: u64,
}

pub async fn land(estate: &Estate, command: Delivery, output: bool) -> Result<()> {
    let Delivery::Land {
        issue,
        authority,
        plan: path,
        command,
        timeout,
    } = command
    else {
        return Err(Error::typed(
            "concord.delivery.command",
            "delivery land received a non-land command",
        ));
    };
    let envelope: Envelope = input::stream(&path, SHAPE)?.ok_or_else(|| {
        Error::typed(
            "concord.delivery.handoff",
            "streamed delivery handoff ended without a plan or preparation refusal",
        )
    })?;
    version(envelope.version)?;
    let plan = match (envelope.plan, envelope.error) {
        (Some(plan), None) => plan,
        (None, Some(error)) => return Err(error.restore()),
        _ => {
            return Err(Error::typed(
                "concord.delivery.envelope",
                "delivery envelope must carry exactly one plan or preparation refusal",
            ));
        }
    };
    if plan.issue != Coordinate::parse(&issue)? {
        return Err(Error::typed(
            "concord.delivery.coordinate",
            "delivery plan Issue or Member does not match the command",
        ));
    }
    if plan.authority.mode() != authority {
        return Err(Error::typed(
            "concord.delivery.authority",
            "land authority does not match the explicitly prepared plan",
        ));
    }
    let flow = Flow {
        estate,
        plan,
        command,
        timeout,
    };
    let mut wait = settle::Wait::new();
    while let Some(pull) = flow.apply(output).await? {
        let mut client = client::Client {
            command: &flow.command,
            repository: &flow.plan.delivery.repository,
            timeout: flow.timeout,
        };
        settle::wait(&mut client, pull, &mut wait).await?;
    }
    Ok(())
}

impl Flow<'_> {
    async fn apply(&self, output: bool) -> Result<Option<i64>> {
        let integration = self.plan.member.integration.guard(self.estate)?;
        let observed = self.observe().await?;
        let mut resumed =
            issue_delivery::resume::<Plumb>(self.estate, &self.plan, &observed.snapshot).await?;
        if !resumed.released {
            branches::clean(
                self.estate,
                &self.plan.member.integration,
                &self.command,
                self.timeout,
            )
            .await?;
        }
        let preparation = self.plan.delivery.preparation();
        let identity = if resumed.released {
            Path::new(&self.plan.member.integration.path)
        } else {
            preparation.root.as_path()
        };
        let mut provider = pull::Service::open(
            &preparation,
            pull::Access {
                identity,
                repository: &self.plan.delivery.repository,
                command: &self.command,
                timeout: self.timeout,
            },
        )
        .await?;
        let mut held = provider.locate(resumed.reference.as_ref()).await?;
        if let Some(number) = held
            .as_ref()
            .filter(|pull| !provider.current(pull))
            .map(|pull| pull.number)
        {
            active(&resumed)?;
            self.mutation(Check::Confirm).await?;
            provider.publish().await?;
            self.mutation(Check::Confirm).await?;
            held = Some(provider.refresh(number).await?);
        }
        if held.is_none() {
            active(&resumed)?;
            self.mutation(Check::Confirm).await?;
            provider.publish().await?;
            self.mutation(Check::Confirm).await?;
            held = Some(provider.raise().await?);
        }
        let mut held = held.expect("missing pull was created");
        if held.state == pull::State::Closed {
            settle::merged(&held)?;
        }
        if !resumed.released {
            resumed.revision = self.attach(&held, resumed.revision).await?;
        }
        if held.state == pull::State::Open {
            active(&resumed)?;
            if provider.pending(held.number).await? {
                return Ok(Some(held.number));
            }
            self.mutation(Check::Execute).await?;
            provider.mark(self.plan.authority.mode()).await?;
            let ready = self.mutation(Check::Confirm).await?;
            let squash = settle::squash(&ready.preparation)?;
            let attempted = provider.settle(held.number, &squash).await;
            held = provider.view(held.number).await?;
            match attempted {
                _ if held.state == pull::State::Merged => {}
                Ok(pull::Merge::Pending) => return Ok(Some(held.number)),
                Ok(pull::Merge::Merged) => {}
                Err(error) => return Err(error),
            }
        }
        let merge = settle::merged(&held)?;
        let reference = reference(&self.plan, &held)?;
        let completion = self
            .estate
            .complete(
                &integration,
                &issue_delivery::Settlement {
                    authority: self.plan.authority.clone(),
                    issue: self.plan.issue.clone(),
                    prepared: self.plan.revision,
                    revision: resumed.revision,
                    member: self.plan.member.clone(),
                    boundary: self.plan.boundary.clone(),
                    reference,
                    base: self.plan.delivery.base.clone(),
                    candidate: self.plan.delivery.candidate.clone(),
                    merge: merge.clone(),
                    pushed: vec![
                        plumb::delivery::Pushed {
                            branch: self.plan.delivery.projection.clone(),
                            head: self.plan.delivery.candidate.clone(),
                        },
                        plumb::delivery::Pushed {
                            branch: self.plan.delivery.branch.clone(),
                            head: self.plan.delivery.source.clone(),
                        },
                    ],
                },
            )
            .await?;
        let report = provider.report(held);
        emit(
            json!({
                "schema": "concord.issue-delivery-land/v3",
                "issue": self.plan.issue,
                "revision": completion.revision,
                "pull": report.pull,
                "candidate": report.candidate,
                "merge": merge,
                "integration": {"head": completion.head},
                "released": completion.released,
                "retired": completion.retired,
                "merged": true,
            }),
            output,
        )
        .map(|_| None)
    }

    async fn mutation(&self, check: Check) -> Result<issue_delivery::Ready> {
        pull::fetch(&self.plan.delivery.root).await?;
        let observed = self.observe().await?;
        let ready = issue_delivery::revalidate::<Plumb>(
            self.estate,
            &self.plan,
            (&observed.snapshot, observed.observed),
            check,
        )
        .await?;
        settle::exact(&ready.preparation).await?;
        Ok(ready)
    }

    async fn observe(&self) -> Result<projection::Observation> {
        let observed =
            super::super::projection::Projection::new(&self.command, 100, 1, self.timeout)
                .delivery(&self.plan.issue)
                .await?;
        if observed.acceptance != self.plan.acceptance {
            return Err(Error::typed(
                "concord.acceptance.changed",
                "delivery acceptance target, declaration, closure or remaining obligations changed; prepare a fresh plan",
            ));
        }
        Ok(observed)
    }

    async fn attach(&self, pull: &pull::Pull, revision: i64) -> Result<i64> {
        let status = self.estate.issue_member_status(&self.plan.issue).await?;
        let (owner, repository) = coordinate(&self.plan.delivery.repository)?;
        if status.references.iter().any(|reference| {
            reference.owner == owner
                && reference.repository == repository
                && reference.number == pull.number
        }) {
            return Ok(self.estate.issue(&self.plan.issue).await?.revision);
        }
        if !status.references.is_empty() {
            return Err(Error::typed(
                "concord.delivery.reference",
                "Member already carries a different pull coordinate",
            ));
        }
        self.estate
            .refer_issue(&IssueDeclaration {
                issue: self.plan.issue.clone(),
                provider: "github".to_string(),
                owner: owner.to_string(),
                repository: repository.to_string(),
                number: pull.number,
                revision,
            })
            .await
    }
}

fn active(resumed: &issue_delivery::Resume) -> Result<()> {
    if resumed.released {
        return Err(Error::typed(
            "concord.delivery.stale",
            "released delivery cannot create or mutate a pull request",
        ));
    }
    Ok(())
}

fn reference(plan: &issue_delivery::Plan, pull: &pull::Pull) -> Result<concord_core::Reference> {
    let (owner, repository) = coordinate(&plan.delivery.repository)?;
    Ok(concord_core::Reference {
        kind: concord_core::ReferenceKind::Change,
        provider: "github".to_string(),
        owner: owner.to_string(),
        repository: repository.to_string(),
        number: pull.number,
    })
}

fn coordinate(repository: &str) -> Result<(&str, &str)> {
    repository.split_once('/').ok_or_else(|| {
        Error::typed(
            "concord.delivery.repository",
            "delivery repository coordinate is malformed",
        )
    })
}
