use super::super::{Estate, Proof};
use super::authority::{self, Authorities, Warrant};
use super::{IssueWorktree, issue_stale};
use crate::{Error, Result, claim, git};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub const SCHEMA: &str = "concord.issue-member-delivery/v2";
pub const READY: &str = "concord.issue-member-delivery-ready/v1";

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub issue: super::super::super::Coordinate,
    pub member: String,
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
    pub authority: Warrant,
    pub delivery: plumb::delivery::Plan,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Member {
    pub key: i64,
    pub name: String,
    pub source: String,
    pub branch: String,
    pub claims: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Ready {
    pub schema: String,
    pub plan: Plan,
    pub preparation: plumb::landing::Preparation,
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
    let context = context(estate, &request.issue, &request.member, request.revision).await?;
    agree(&context, &request.snapshot)?;
    let pull = narrative(&context, &request.snapshot, &request.outcome)?;
    let (authority, warrant) = authority::select::<A>(
        &context.path,
        &context.boundary.head,
        "concord.delivery.authority",
    )?;
    let delivery = plumb::delivery::prepare(
        plumb::delivery::Request {
            root: &context.path,
            repository: &request.snapshot.repository,
            issue: &request.snapshot,
            observed: request.observed,
            base: &request.base,
            pull: &pull,
        },
        &authority,
    )
    .map_err(refused)?;
    Ok(Plan {
        schema: SCHEMA.to_string(),
        issue: context.issue,
        node: context.node,
        revision: context.revision,
        member: member(&context.member),
        boundary: context.boundary,
        authority: warrant,
        delivery,
    })
}

pub async fn revalidate<A: Authorities>(
    estate: &Estate,
    plan: &Plan,
    snapshot: &plumb::delivery::Snapshot,
    observed: u64,
) -> Result<Ready> {
    if plan.schema != SCHEMA {
        return Err(Error::typed(
            "concord.delivery.schema",
            format!("delivery plan schema {} is not {SCHEMA}", plan.schema),
        ));
    }
    let context = context(estate, &plan.issue, &plan.member.name, plan.revision).await?;
    agree(&context, snapshot)?;
    if context.node != plan.node
        || member(&context.member) != plan.member
        || context.boundary != plan.boundary
    {
        return Err(Error::typed(
            "concord.delivery.stale",
            "Issue, Member, or Boundary identity changed after delivery preparation",
        ));
    }
    let authority = authority::keep::<A>(&plan.authority, "concord.delivery.authority")?;
    let ready = plumb::delivery::revalidate(
        plumb::delivery::Request {
            root: &context.path,
            repository: &plan.delivery.repository,
            issue: snapshot,
            observed,
            base: &plan.delivery.base,
            pull: &plan.delivery.pull,
        },
        &plan.delivery,
        &authority,
    )
    .map_err(refused)?;
    Ok(Ready {
        schema: READY.to_string(),
        plan: plan.clone(),
        preparation: ready.preparation().clone(),
    })
}

async fn context(
    estate: &Estate,
    issue: &super::super::super::Coordinate,
    name: &str,
    revision: i64,
) -> Result<Context> {
    estate.ensure().await?;
    let anchor = estate.issue(issue).await?;
    issue_stale(anchor.revision, revision)?;
    let member = estate.issue_member(issue, name).await?;
    let boundary = member.proof.clone().ok_or_else(|| {
        Error::typed(
            "concord.delivery.boundary",
            "Member has no current Boundary proof",
        )
    })?;
    let path = estate.issue_path(&anchor, &member.name)?;
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
    if snapshot.node != context.node
        || snapshot.repository != format!("{}/{}", context.issue.owner, context.issue.repository)
        || i64::try_from(snapshot.number).ok() != Some(context.issue.number)
    {
        return Err(Error::typed(
            "concord.delivery.issue",
            format!("Issue snapshot does not match execution anchor {coordinate}"),
        ));
    }
    Ok(())
}

fn narrative(
    context: &Context,
    snapshot: &plumb::delivery::Snapshot,
    outcome: &str,
) -> Result<plumb::delivery::Narrative> {
    let outcome = outcome.trim();
    if outcome.is_empty() {
        return Err(Error::typed(
            "concord.delivery.outcome",
            "Issue Outcome section is empty",
        ));
    }
    let coordinate = format!("{}#{}", snapshot.repository, snapshot.number);
    let claims = context
        .member
        .claims
        .iter()
        .map(|path| format!("- `{path}`"))
        .collect::<Vec<_>>()
        .join("\n");
    Ok(plumb::delivery::Narrative {
        title: snapshot.title.clone(),
        body: format!(
            "Refs {coordinate}\n\n## Outcome\n\n{outcome}\n\n## Change\n\nMember `{}` contributes `{}` from branch `{}`.\n\n## Verification\n\nBoundary `{}` proves commit `{}` under `{}`.\n\n## Boundary\n\n{claims}",
            context.member.name,
            context.boundary.head,
            context.member.branch,
            context.boundary.key,
            context.boundary.head,
            context.boundary.plumb,
        ),
    })
}

fn agreement(source: &Path, path: &Path, member: &IssueWorktree, proof: &Proof) -> Result<()> {
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
    if !proof.current(&head, &digest) {
        return Err(Error::typed(
            "concord.delivery.boundary",
            "Member Boundary proof is stale",
        ));
    }
    Ok(())
}

fn member(member: &IssueWorktree) -> Member {
    Member {
        key: member.key,
        name: member.name.clone(),
        source: member.source.clone(),
        branch: member.branch.clone(),
        claims: member.claims.clone(),
    }
}

fn refused(refusal: plumb::landing::Refusal) -> Error {
    Error::typed(
        format!("concord.delivery.{}", refusal.kind),
        refusal.message,
    )
}
