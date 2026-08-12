mod claim;
mod member;
mod memory;

use crate::git;
use crate::path::at;
use crate::{Domain, Legacy, Result, Space};
use serde::Serialize;
use std::collections::BTreeSet;
use std::path::Path;

#[derive(Clone, Debug, Serialize)]
pub struct Audit {
    pub target: String,
    pub faults: Vec<Fault>,
    pub observations: Vec<Advisory>,
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
        let mut audit = Audit {
            target: self.path().display().to_string(),
            faults: Vec::new(),
            observations: Vec::new(),
        };
        for domain in self.domains()? {
            merge(&mut audit, domain.audit()?);
        }
        claim::conflicts(self, &mut audit)?;
        Ok(audit)
    }
}

impl Domain {
    fn audit(&self) -> Result<Audit> {
        let mut audit = Audit {
            target: self.name().to_string(),
            faults: Vec::new(),
            observations: Vec::new(),
        };
        permission(&mut audit, &self.tasks(), 0o700)?;
        permission(&mut audit, &self.manifest(), 0o600)?;
        let registry = self.registry()?;
        let declared = registry
            .task
            .iter()
            .map(|task| task.name.as_str())
            .collect::<BTreeSet<_>>();
        for task in &registry.task {
            merge(&mut audit, self.task(&task.name)?.audit()?);
        }
        for entry in std::fs::read_dir(self.tasks())? {
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

impl Legacy {
    #[locus::trace(with = crate::observation::view())]
    fn audit(&self) -> Result<Audit> {
        let mut audit = Audit {
            target: self.identity(),
            faults: Vec::new(),
            observations: Vec::new(),
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
        let version = self.domain().registry()?.version;
        for member in &self.task().repo {
            member::inspect(&mut audit, self, member, version)?;
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
        Ok(audit)
    }
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
}
