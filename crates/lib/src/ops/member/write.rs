use super::*;

impl Space {
    pub fn member_claim(
        &self,
        identity: &str,
        name: &str,
        write: &[String],
        apply: bool,
    ) -> Result<Plan> {
        let task = self.resolve(identity)?;
        version(&task)?;
        task.ensure_exact()?;
        let member = task
            .task()
            .repo
            .iter()
            .find(|member| member.name == name)
            .ok_or_else(|| Error::new(format!("member not found: {name}")))?;
        let combined = union(&member.write, write)?;
        let source = task.source(&member.source)?;
        claim::available(
            self,
            claim::Wanted {
                task: &task.identity(),
                member: name,
                source: &source,
                write: &combined,
            },
        )?;
        let actions = vec![action(
            "claim",
            &task.domain().registry_path(),
            format!("member {name} write {}", combined.join(", ")),
        )];
        if apply {
            let _lock = self.lock()?;
            let task = self.resolve(identity)?;
            version(&task)?;
            task.ensure_exact()?;
            expand(self, &task, name, write)?;
        }
        Ok(Plan::new("member.claim", actions, apply))
    }
}

pub(super) fn version(task: &TaskRef) -> Result<()> {
    let version = task.domain().registry()?.version;
    if version >= 2 {
        return Ok(());
    }
    Err(Error::new(format!(
        "domain {} uses registry version {version}; migrate it before adding or expanding members",
        task.domain().name(),
    )))
}

fn union(held: &[String], added: &[String]) -> Result<Vec<String>> {
    let mut combined = held.to_vec();
    combined.extend_from_slice(added);
    claim::normalize(&combined)
}

fn expand(space: &Space, task: &TaskRef, name: &str, write: &[String]) -> Result<()> {
    let domain = task.domain();
    let mut snapshot = domain.read()?;
    let member = snapshot
        .registry
        .task
        .iter_mut()
        .find(|held| held.name == task.task().name)
        .and_then(|held| held.repo.iter_mut().find(|held| held.name == name))
        .ok_or_else(|| Error::new("member disappeared during claim expansion"))?;
    let combined = union(&member.write, write)?;
    let source = task.source(&member.source)?;
    claim::available(
        space,
        claim::Wanted {
            task: &task.identity(),
            member: name,
            source: &source,
            write: &combined,
        },
    )?;
    if combined == member.write {
        return Ok(());
    }
    member.write = combined;
    member.boundary = None;
    domain.write(&snapshot.raw, &snapshot.registry)
}
