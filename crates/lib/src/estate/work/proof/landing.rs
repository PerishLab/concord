use super::super::super::World;
use super::super::{Estate, Proof, Worktree, active, stale};
use crate::{Error, PLUMB, Result, claim, git};
use plumb::guard::{Authority, Expected};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub const SCHEMA: &str = "concord.member-landing/v1";
pub const READY: &str = "concord.member-ready/v1";

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Guard {
    pub schema: String,
    pub tree: String,
    pub digest: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub task: String,
    pub member: String,
    pub revision: i64,
    pub base: String,
    pub title: String,
    pub body: String,
    pub guard: Guard,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Plan {
    pub schema: String,
    pub task: String,
    pub revision: i64,
    pub member: Member,
    pub boundary: Boundary,
    pub guard: Guard,
    pub landing: plumb::landing::Preparation,
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

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Boundary {
    pub key: i64,
    pub schema: String,
    pub plumb: String,
    pub base: String,
    pub head: String,
    pub claim: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Ready {
    pub schema: String,
    pub plan: Plan,
}

struct Context {
    task: String,
    revision: i64,
    member: Worktree,
    boundary: Proof,
    path: PathBuf,
}

pub async fn prepare(estate: &Estate, request: &Request) -> Result<Plan> {
    let context = context(estate, &request.task, &request.member, request.revision).await?;
    let inspected = plumb::landing::Request {
        root: &context.path,
        base: &request.base,
        title: &request.title,
        body: &request.body,
    }
    .inspect()
    .map_err(refused)?;
    let verified = authority(inspected.root(), inspected.source(), &request.guard)?;
    let landing = inspected.prepare(verified).map_err(refused)?;
    Ok(Plan {
        schema: SCHEMA.to_string(),
        task: context.task,
        revision: context.revision,
        member: member(&context.member),
        boundary: boundary(&context.boundary),
        guard: request.guard.clone(),
        landing,
    })
}

pub async fn revalidate(estate: &Estate, plan: &Plan) -> Result<Ready> {
    if plan.schema != SCHEMA {
        return Err(Error::typed(
            "concord.landing.schema",
            format!("landing plan schema {} is not {SCHEMA}", plan.schema),
        ));
    }
    let context = context(estate, &plan.task, &plan.member.name, plan.revision).await?;
    if member(&context.member) != plan.member || boundary(&context.boundary) != plan.boundary {
        return Err(Error::typed(
            "concord.landing.stale",
            "Member or Boundary identity changed after landing preparation",
        ));
    }
    let verified = authority(&context.path, &plan.landing.source, &plan.guard)?;
    let landing = plumb::landing::Request {
        root: &context.path,
        base: &plan.landing.base,
        title: &plan.landing.title,
        body: &plan.landing.body,
    }
    .revalidate(&plan.landing, verified)
    .map_err(refused)?
    .take();
    let mut plan = plan.clone();
    plan.landing = landing;
    Ok(Ready {
        schema: READY.to_string(),
        plan,
    })
}

async fn context(estate: &Estate, task: &str, name: &str, revision: i64) -> Result<Context> {
    estate.ensure().await?;
    let world = World::load(estate).await?;
    let task = world.node(task)?;
    active(task.life, &task.identity())?;
    stale(task.revision, revision)?;
    let member = estate.member(&task.identity(), name).await?;
    let boundary = member.proof.clone().ok_or_else(|| {
        Error::typed(
            "concord.landing.boundary",
            "Member has no current Boundary proof",
        )
    })?;
    let path = estate.path(&task.domain, &task.name, &member.name);
    let source = estate.source(&member)?;
    agreement(&source, &path, &member, &boundary)?;
    Ok(Context {
        task: task.identity(),
        revision: task.revision,
        member,
        boundary,
        path,
    })
}

fn agreement(source: &Path, path: &Path, member: &Worktree, proof: &Proof) -> Result<()> {
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
    let found = (
        proof.schema.as_str(),
        proof.plumb.as_str(),
        proof.head.as_str(),
        proof.claim.as_str(),
    );
    let head = git::at(path).head()?;
    let digest = claim::digest(&member.claims);
    let expected = (
        plumb::boundary::SCHEMA,
        PLUMB,
        head.as_str(),
        digest.as_str(),
    );
    if found != expected {
        return Err(Error::typed(
            "concord.landing.boundary",
            "Member Boundary proof is stale",
        ));
    }
    Ok(())
}

fn authority(root: &Path, source: &str, guard: &Guard) -> Result<plumb::guard::Verified> {
    let authority =
        Authority::released().map_err(|error| Error::typed("concord.landing.authority", error))?;
    authority
        .verify(
            root,
            source,
            &Expected::new(&guard.schema, &guard.tree, &guard.digest),
        )
        .map_err(|error| Error::typed("concord.landing.guard", error))
}

fn member(member: &Worktree) -> Member {
    Member {
        key: member.key,
        name: member.name.clone(),
        source: member.source.clone(),
        branch: member.branch.clone(),
        claims: member.claims.clone(),
    }
}

fn boundary(proof: &Proof) -> Boundary {
    Boundary {
        key: proof.key,
        schema: proof.schema.clone(),
        plumb: proof.plumb.clone(),
        base: proof.base.clone(),
        head: proof.head.clone(),
        claim: proof.claim.clone(),
    }
}

fn refused(refusal: plumb::landing::Refusal) -> Error {
    Error::typed(format!("concord.landing.{}", refusal.kind), refusal.message)
}
