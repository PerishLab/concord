mod memory;
mod path;

use super::{Census, Evidence, RELEASE, load};
use crate::{Entry, Error, Fact, Registry, Result, Space, Task};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

pub(super) struct Source {
    pub realms: Vec<Realm>,
    pub census: Census,
}

pub(super) struct Realm {
    pub name: String,
    pub registry: Registry,
    pub works: Vec<Work>,
}

pub(super) struct Work {
    pub task: Task,
    pub facts: Vec<Fact>,
    pub phases: Vec<Vec<Entry>>,
    pub artifacts: Vec<String>,
}

impl Source {
    pub fn read(space: &Space) -> Result<Self> {
        let mut realms = Vec::new();
        let mut evidence = Vec::new();
        for domain in space.domains()? {
            let registry = domain.registry()?;
            if registry.version != 3 {
                return Err(Error::typed(
                    "concord.migration.registry",
                    format!(
                        "Domain {} needs registry version 3 before estate migration",
                        domain.name()
                    ),
                ));
            }
            let mut trace = path::Trace::new(space.path(), &mut evidence);
            trace.one(&domain.tasks(), "directory")?;
            trace.one(&domain.manifest(), "registry")?;
            let mut works = Vec::new();
            for task in &registry.task {
                let task = domain.task(&task.name)?;
                territory(&task)?;
                trace.one(&task.path(), "directory")?;
                let (main, phases, artifacts) = memory::read(&mut trace, &task)?;
                let facts = main
                    .as_deref()
                    .map(load::facts)
                    .transpose()?
                    .unwrap_or_default();
                let phases = phases
                    .iter()
                    .map(|phase| load::entries(phase))
                    .collect::<Result<Vec<_>>>()?;
                works.push(Work {
                    task: task.task().clone(),
                    facts,
                    phases,
                    artifacts,
                });
            }
            realms.push(Realm {
                name: domain.name().to_string(),
                registry,
                works,
            });
        }
        evidence.sort_by_key(|entry| (entry.path.clone(), entry.kind.clone()));
        let mut census = count(&realms, evidence)?;
        census.fingerprint = fingerprint(&census.evidence);
        Ok(Self { realms, census })
    }
}

fn territory(task: &crate::Legacy) -> Result<()> {
    let mut known = task
        .task()
        .repo
        .iter()
        .map(|member| member.name.as_str())
        .collect::<BTreeSet<_>>();
    known.insert(".task");
    for entry in std::fs::read_dir(task.path())? {
        let entry = entry?;
        let name = entry.file_name();
        if !name.to_str().is_some_and(|name| known.contains(name)) {
            return Err(Error::typed(
                "concord.migration.territory",
                format!("unknown Task territory: {}", entry.path().display()),
            ));
        }
    }
    Ok(())
}

fn count(realms: &[Realm], evidence: Vec<Evidence>) -> Result<Census> {
    let mut census = Census {
        release: RELEASE.to_string(),
        fingerprint: String::new(),
        domain: realms.len(),
        repository: 0,
        task: 0,
        member: 0,
        claim: 0,
        boundary: 0,
        dependency: 0,
        current: 0,
        phase: 0,
        artifact: 0,
        evidence,
    };
    for realm in realms {
        census.repository += realm.registry.repo.len();
        census.task += realm.works.len();
        for work in &realm.works {
            census.member += work.task.repo.len();
            census.claim += work
                .task
                .repo
                .iter()
                .map(|member| member.write.len())
                .sum::<usize>();
            census.boundary += work
                .task
                .repo
                .iter()
                .filter(|member| member.boundary.is_some())
                .count();
            census.dependency += work.task.todo.len();
            census.current += work.task.extra.len() + work.facts.len();
            census.phase += work.phases.len();
            census.artifact += work.artifacts.len();
        }
    }
    Ok(census)
}

fn fingerprint(evidence: &[Evidence]) -> String {
    let mut hash = Sha256::new();
    for held in evidence {
        hash.update(held.path.as_bytes());
        hash.update([0]);
        hash.update(held.kind.as_bytes());
        hash.update([0]);
        hash.update(held.bytes.to_le_bytes());
        hash.update(held.mode.unwrap_or(0).to_le_bytes());
        if let Some(digest) = &held.digest {
            hash.update(digest.as_bytes());
        }
        hash.update([0xff]);
    }
    format!("{:x}", hash.finalize())
}
