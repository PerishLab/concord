mod memory;
mod preflight;
pub(crate) mod resource;

use crate::git;
use crate::path::at;
use crate::{Domain, Result, Space, TaskRef};
use serde::Serialize;
use std::collections::BTreeSet;
use std::path::Path;

pub use preflight::{LandingProof, MemberPreflight, MemberProof, Preflight};
pub use resource::{
    Filesystem, Footprint, HostMemory, ImportPreflight, Inodes, Observation, Status, TaskResources,
};

#[derive(Clone, Debug, Serialize)]
pub struct Audit {
    pub target: String,
    pub faults: Vec<Fault>,
    pub observations: Vec<Advisory>,
    pub resources: Vec<TaskResources>,
}

#[derive(Clone, Debug, Serialize)]
pub struct Fault {
    pub kind: String,
    pub path: String,
    pub message: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct Advisory {
    pub kind: String,
    pub path: String,
    pub message: String,
}

const HYGIENE: [&str; 3] = ["permission", "memory", "resource"];

impl Fault {
    pub fn gates(&self) -> bool {
        !HYGIENE.contains(&self.kind.as_str())
    }
}

impl Audit {
    pub fn ok(&self) -> bool {
        self.faults.is_empty()
    }

    pub fn agrees(&self) -> bool {
        !self.faults.iter().any(Fault::gates)
    }

    fn fault(&mut self, kind: &str, path: &Path, message: impl Into<String>) {
        self.faults.push(Fault {
            kind: kind.to_string(),
            path: path.display().to_string(),
            message: message.into(),
        });
    }

    fn observe(&mut self, kind: &str, path: &Path, message: impl Into<String>) {
        self.observations.push(Advisory {
            kind: kind.to_string(),
            path: path.display().to_string(),
            message: message.into(),
        });
    }
}

impl Space {
    pub fn audit(&self) -> Result<Audit> {
        self.audit_with_resources(Some(&resource::host()))
    }

    fn audit_with_resources(&self, host: Option<&Observation<HostMemory>>) -> Result<Audit> {
        let mut audit = Audit {
            target: self.path().display().to_string(),
            faults: Vec::new(),
            observations: Vec::new(),
            resources: Vec::new(),
        };
        for domain in self.domains()? {
            merge(&mut audit, domain.audit_with_resources(host)?);
        }
        Ok(audit)
    }
}

impl Domain {
    pub fn audit(&self) -> Result<Audit> {
        self.audit_with_resources(Some(&resource::host()))
    }

    fn audit_with_resources(&self, host: Option<&Observation<HostMemory>>) -> Result<Audit> {
        let mut audit = Audit {
            target: self.name().to_string(),
            faults: Vec::new(),
            observations: Vec::new(),
            resources: Vec::new(),
        };
        permission(&mut audit, &self.tasks_path(), 0o700)?;
        permission(&mut audit, &self.registry_path(), 0o600)?;
        let registry = self.registry()?;
        let declared = registry
            .task
            .iter()
            .map(|task| task.name.as_str())
            .collect::<BTreeSet<_>>();
        for task in &registry.task {
            merge(
                &mut audit,
                self.task(&task.name)?.audit_with_resources(host)?,
            );
        }
        for entry in std::fs::read_dir(self.tasks_path())? {
            let entry = entry?;
            if !entry.file_type()?.is_dir() {
                continue;
            }
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if !declared.contains(name.as_ref()) {
                audit.fault("registry", &entry.path(), "task root has no registry entry");
            }
        }
        Ok(audit)
    }
}

impl TaskRef {
    pub fn audit(&self) -> Result<Audit> {
        self.audit_with_resources(Some(&resource::host()))
    }

    #[locus::trace(with = crate::observation::view())]
    pub(crate) fn agreement(&self) -> Result<Audit> {
        self.audit_with_resources(None)
    }

    #[locus::trace(with = crate::observation::view())]
    fn audit_with_resources(&self, host: Option<&Observation<HostMemory>>) -> Result<Audit> {
        let mut audit = Audit {
            target: self.identity(),
            faults: Vec::new(),
            observations: Vec::new(),
            resources: Vec::new(),
        };
        let root = self.path();
        if !root.is_dir() {
            audit.fault("presence", &root, "declared task root is missing");
            return Ok(audit);
        }
        permission(&mut audit, &root, 0o700)?;
        let declared = self
            .task()
            .repo
            .iter()
            .map(|member| member.name.as_str())
            .collect::<BTreeSet<_>>();
        for member in &self.task().repo {
            member_audit(&mut audit, self, member)?;
        }
        for entry in std::fs::read_dir(&root)? {
            let entry = entry?;
            if entry.file_name() == ".task" || !entry.file_type()?.is_dir() {
                continue;
            }
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if entry.path().join(".git").exists() && !declared.contains(name.as_ref()) {
                audit.fault(
                    "registry",
                    &entry.path(),
                    "Git worktree under task root is undeclared",
                );
            }
        }
        memory::permissions(&mut audit, &root.join(".task"))?;
        if let Some(host) = host {
            audit.resources.push(resource::inspect(self, host));
        }
        Ok(audit)
    }

    #[locus::trace(with = crate::observation::view())]
    pub(crate) fn ensure_exact(&self) -> Result<()> {
        let audit = self.agreement()?;
        if audit.agrees() {
            Ok(())
        } else {
            Err(crate::Error::new(format!(
                "task protocol mismatch: {} fault(s); run concord audit {}",
                audit.faults.iter().filter(|fault| fault.gates()).count(),
                self.identity()
            )))
        }
    }
}

#[locus::trace(with = crate::observation::view())]
fn member_audit(audit: &mut Audit, task: &TaskRef, member: &crate::Member) -> Result<()> {
    let path = task.member_path(&member.name);
    if !path.is_dir() {
        audit.fault("presence", &path, "declared member path is missing");
        return Ok(());
    }
    let source = match task.source(&member.source) {
        Ok(source) => source,
        Err(error) => {
            audit.fault("source", &path, error.to_string());
            return Ok(());
        }
    };
    let source_id = git::at(&source).identity();
    let seat = git::at(&path).seat();
    match (&source_id, &seat) {
        (Ok(source_id), Ok(seat)) if source_id != &seat.identity => {
            audit.fault(
                "identity",
                &path,
                format!(
                    "member Git identity {} differs from source {}",
                    seat.identity.display(),
                    source_id.display()
                ),
            );
        }
        (Err(error), _) => audit.fault("source", &source, error.to_string()),
        (_, Err(error)) => audit.fault("worktree", &path, error.to_string()),
        _ => {}
    }
    if source.is_dir() && !git::at(&source).registered(&path)? {
        audit.fault(
            "worktree",
            &path,
            "member path is absent from source Git worktree metadata",
        );
    }
    match seat {
        Ok(seat) => {
            let expected = member.branch(&task.task().name);
            if seat.branch != expected {
                audit.fault(
                    "branch",
                    &path,
                    format!(
                        "member branch {} differs from registry {expected}",
                        seat.branch
                    ),
                );
            }
        }
        Err(error) => {
            audit.fault("branch", &path, error.to_string());
        }
    }
    Ok(())
}

fn permission(audit: &mut Audit, path: &Path, wanted: u32) -> Result<()> {
    if !path.exists() {
        return Ok(());
    }
    if let Some(held) = at(path).held()?
        && held != wanted
    {
        audit.fault(
            "permission",
            path,
            format!("mode {held:04o} must be {wanted:04o}"),
        );
    }
    Ok(())
}

fn merge(target: &mut Audit, source: Audit) {
    target.faults.extend(source.faults);
    target.observations.extend(source.observations);
    target.resources.extend(source.resources);
}
