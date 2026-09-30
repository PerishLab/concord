use super::super::{Estate, Proof};
use super::authority::Authorities;
use super::{IssueWorktree, issue_stale};
use crate::{Error, Reference, Result, claim, git};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[path = "../delivery/engine.rs"]
mod engine;
#[path = "../delivery/gate.rs"]
mod gate;
#[path = "../delivery/model.rs"]
mod model;
#[path = "../delivery/process.rs"]
mod process;

pub use model::{Authority, Candidate, Mode, Preparation};

pub use super::authority::delivery::{Resume, resume};
pub use super::proof::delivery::{Completion, Settlement};

pub const SCHEMA: &str = "concord.issue-member-delivery/v5";
pub const READY: &str = "concord.issue-member-delivery-ready/v4";

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub authority: Mode,
    pub issue: super::super::super::Coordinate,
    pub revision: i64,
    pub base: String,
    pub snapshot: plumb::delivery::Snapshot,
    pub observed: u64,
    pub outcome: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Plan {
    pub schema: String,
    pub issue: super::super::super::Coordinate,
    pub node: String,
    pub revision: i64,
    pub member: Member,
    pub boundary: Proof,
    pub authority: Authority,
    pub delivery: Candidate,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Member {
    pub key: i64,
    pub integration: super::super::Integration,
    pub branch: String,
    pub base: String,
    pub claims: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Ready {
    pub schema: String,
    pub plan: Plan,
    pub preparation: Preparation,
    pub revision: i64,
    pub reference: Option<Reference>,
}

struct Context {
    issue: super::super::super::Coordinate,
    node: String,
    revision: i64,
    member: IssueWorktree,
    boundary: Proof,
    path: PathBuf,
}

pub async fn prepare<A: Authorities>(estate: &Estate, request: &Request) -> Result<Plan> {
    let context = context(estate, &request.issue, request.revision).await?;
    agree(&context, &request.snapshot)?;
    let pull = narrative(&context, request)?;
    let (authority, delivery) = engine::prepare::<A>(&context, request, &pull)?;
    Ok(Plan {
        schema: SCHEMA.to_string(),
        issue: context.issue,
        node: context.node,
        revision: context.revision,
        member: member(&context.member),
        boundary: context.boundary,
        authority,
        delivery,
    })
}

pub async fn revalidate<A: Authorities>(
    estate: &Estate,
    plan: &Plan,
    snapshot: &plumb::delivery::Snapshot,
    observed: u64,
) -> Result<Ready> {
    let resumed = resume::<A>(estate, plan, snapshot).await?;
    if resumed.released {
        return Err(Error::typed(
            "concord.delivery.stale",
            "delivery is already released and cannot perform another provider mutation",
        ));
    }
    let context = context(estate, &plan.issue, resumed.revision).await?;
    engine::revalidate::<A>(&context, plan, snapshot, observed)?;
    Ok(Ready {
        schema: READY.to_string(),
        plan: plan.clone(),
        preparation: plan.delivery.preparation(),
        revision: resumed.revision,
        reference: resumed.reference,
    })
}

async fn context(
    estate: &Estate,
    issue: &super::super::super::Coordinate,
    revision: i64,
) -> Result<Context> {
    estate.agreed().await?;
    let anchor = estate.issue(issue).await?;
    issue_stale(anchor.revision, revision)?;
    let member = estate.issue_member(issue).await?;
    let boundary = member.proof.clone().ok_or_else(|| {
        Error::typed(
            "concord.delivery.boundary",
            "Member has no current Boundary proof",
        )
    })?;
    let path = estate.issue_path(&anchor)?;
    let source = estate.issue_source(&member)?;
    agreement(&source, &path, &member, &boundary)?;
    Ok(Context {
        issue: anchor.coordinate,
        node: anchor.node,
        revision: anchor.revision,
        member,
        boundary,
        path,
    })
}

fn agree(context: &Context, snapshot: &plumb::delivery::Snapshot) -> Result<()> {
    let coordinate = format!(
        "{}/{}#{}",
        context.issue.owner, context.issue.repository, context.issue.number
    );
    let repository = format!("{}/{}", context.issue.owner, context.issue.repository);
    if snapshot.node != context.node || snapshot.repository != repository {
        return Err(issue(coordinate));
    }
    if i64::try_from(snapshot.number).ok() != Some(context.issue.number) {
        return Err(issue(coordinate));
    }
    Ok(())
}

fn issue(coordinate: String) -> Error {
    Error::typed(
        "concord.delivery.issue",
        format!("Issue snapshot does not match execution anchor {coordinate}"),
    )
}

fn narrative(context: &Context, request: &Request) -> Result<plumb::delivery::Narrative> {
    let outcome = request.outcome.trim();
    if outcome.is_empty() {
        return Err(Error::typed(
            "concord.delivery.outcome",
            "Issue Outcome section is empty",
        ));
    }
    let mut pull = member(&context.member).narrative(&request.snapshot, &context.boundary, outcome);
    if request.authority == Mode::WharfNative {
        pull.body.push_str(&format!(
            "\n\nRepository authority: `{}`\n\nFixed native gate: `python3 -B -m scripts.selfcheck` and `python3 -B -m unittest discover -s tests -t . -q`, with no inherited provider environment. Exact source/tree and Python/Git identities are carried by native candidate evidence; this is not a Plumb Guard proof.",
            gate::AUTHORITY,
        ));
    }
    Ok(pull)
}

impl Member {
    fn narrative(
        &self,
        snapshot: &plumb::delivery::Snapshot,
        boundary: &Proof,
        outcome: &str,
    ) -> plumb::delivery::Narrative {
        let coordinate = format!("{}#{}", snapshot.repository, snapshot.number);
        let claims = self
            .claims
            .iter()
            .map(|path| format!("- `{path}`"))
            .collect::<Vec<_>>()
            .join("\n");
        plumb::delivery::Narrative {
            title: snapshot.title.clone(),
            body: format!(
                "Refs {coordinate}\n\n## Outcome\n\n{outcome}\n\n## Change\n\nThe Issue Member contributes `{}` from branch `{}`.\n\n## Verification\n\nBoundary `{}` proves commit `{}` under `{}`.\n\n## Boundary\n\n{claims}",
                boundary.head, self.branch, boundary.key, boundary.head, boundary.plumb,
            ),
        }
    }
}

pub(super) fn agreement(
    source: &Path,
    path: &Path,
    member: &IssueWorktree,
    proof: &Proof,
) -> Result<()> {
    if !git::at(source).registered(path)? {
        return Err(Error::typed(
            "concord.member.disagreement",
            "Member worktree is not registered by its source repository",
        ));
    }
    if git::at(path).branch()? != member.branch {
        return Err(Error::typed(
            "concord.member.disagreement",
            "Member worktree is not on its declared branch",
        ));
    }
    if !git::at(path).clean()? {
        return Err(Error::typed(
            "concord.member.dirty",
            "Member worktree is not clean",
        ));
    }
    let head = git::at(path).head()?;
    let digest = claim::digest(&member.claims);
    if let Some(reason) = proof.stale(&head, &digest) {
        return Err(Error::typed("concord.delivery.boundary", reason));
    }
    Ok(())
}

pub(super) fn member(member: &IssueWorktree) -> Member {
    Member {
        key: member.key,
        integration: member.integration.clone(),
        branch: member.branch.clone(),
        base: member.base.clone(),
        claims: member.claims.clone(),
    }
}

fn refused(refusal: plumb::landing::Refusal) -> Error {
    Error::typed(
        format!("concord.delivery.{}", refusal.kind),
        refusal.message,
    )
}
