use crate::{BoundaryProof, Error, Result, Space, claim, git};
use serde::Serialize;

pub const PLUMB_VERSION: &str = env!("CONCORD_PLUMB_VERSION");

#[derive(Clone, Debug, Serialize)]
pub struct BoundaryCheck {
    pub task: String,
    pub member: String,
    pub proof: BoundaryProof,
    pub changed: Vec<String>,
}

impl Space {
    pub fn member_boundary(&self, identity: &str, name: &str) -> Result<BoundaryCheck> {
        let _lock = self.lock()?;
        let task = self.resolve(identity)?;
        let version = task.domain().registry()?.version;
        if version == 1 {
            return Err(Error::new(format!(
                "domain {} uses registry version {version}; migrate it before proving boundaries",
                task.domain().name(),
            )));
        }
        task.ensure_exact()?;
        let member = task
            .task()
            .repo
            .iter()
            .find(|member| member.name == name)
            .ok_or_else(|| Error::new(format!("member not found: {name}")))?;
        let path = task.member_path(name);
        let source = task.source(&member.source)?;
        claim::available(
            self,
            claim::Wanted {
                task: &task.identity(),
                member: name,
                source: &source,
                write: &member.write,
            },
        )?;
        if !git::at(&source).clean()? {
            return Err(Error::new("integration checkout is not clean"));
        }
        let head = git::at(&path).head()?;
        let source_head = git::at(&source).head()?;
        let base = git::at(&path).merge_base(&head, &source_head)?;
        let report = plumb::boundary::check(plumb::boundary::Request {
            root: &path,
            base: &base,
            head: &head,
            write: &member.write,
        })
        .map_err(|refusal| Error::new(format!("boundary proof refused: {refusal}")))?;
        if !report.ok {
            return Err(Error::new(format!(
                "boundary proof found changes outside the claim: {}",
                report.outside.join(", ")
            )));
        }
        let proof = BoundaryProof {
            schema: plumb::boundary::SCHEMA.to_string(),
            plumb: PLUMB_VERSION.to_string(),
            base,
            head,
            claim: claim::digest(&member.write),
        };
        let domain = task.domain();
        let mut snapshot = domain.read()?;
        let held = snapshot
            .registry
            .task
            .iter_mut()
            .find(|held| held.name == task.task().name)
            .and_then(|held| held.repo.iter_mut().find(|held| held.name == name))
            .ok_or_else(|| Error::new("member disappeared during boundary proof"))?;
        if held.write != member.write {
            return Err(Error::new("member claim changed during boundary proof"));
        }
        held.boundary = Some(proof.clone());
        domain.write(&snapshot.raw, &snapshot.registry)?;
        Ok(BoundaryCheck {
            task: task.identity(),
            member: name.to_string(),
            proof,
            changed: report.changed,
        })
    }
}

pub(crate) fn valid(proof: &BoundaryProof, write: &[String], head: &str) -> bool {
    if proof.schema != plumb::boundary::SCHEMA || proof.plumb != PLUMB_VERSION {
        return false;
    }
    proof.head == head && proof.claim == claim::digest(write)
}
