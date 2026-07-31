use super::*;
use crate::{MigrationClaim, Registry, claim, git};

impl Space {
    pub fn domain_migrate(
        &self,
        name: &str,
        claims: &[MigrationClaim],
        apply: bool,
    ) -> Result<Plan> {
        let domain = self.domain(name)?;
        let snapshot = domain.read()?;
        let migrated = migration(&snapshot.registry, claims)?;
        conflicts(self, &domain, &migrated)?;
        let actions = actions(&domain, &migrated);
        if apply {
            let _lock = self.lock()?;
            let domain = self.domain(name)?;
            let snapshot = domain.read()?;
            let migrated = migration(&snapshot.registry, claims)?;
            conflicts(self, &domain, &migrated)?;
            domain.write(&snapshot.raw, &migrated)?;
        }
        Ok(Plan::new("domain.migrate", actions, apply))
    }
}

fn actions(domain: &Domain, registry: &Registry) -> Vec<super::super::Action> {
    let mut actions = Vec::new();
    for task in &registry.task {
        for member in &task.repo {
            actions.push(action(
                "claim",
                &domain.registry_path(),
                format!(
                    "member {}/{} write {}",
                    task.name,
                    member.name,
                    member.write.join(", ")
                ),
            ));
        }
    }
    actions.push(action(
        "write",
        &domain.registry_path(),
        "registry version 2",
    ));
    actions
}

fn migration(registry: &Registry, claims: &[MigrationClaim]) -> Result<Registry> {
    if registry.version != 1 {
        return Err(Error::new(format!(
            "registry version {} does not require version 2 migration",
            registry.version
        )));
    }
    let mut supplied: BTreeMap<(String, String), Vec<String>> = BTreeMap::new();
    for held in claims {
        component("task name", &held.task)?;
        component("member name", &held.member)?;
        supplied
            .entry((held.task.clone(), held.member.clone()))
            .or_default()
            .push(held.write.clone());
    }
    let mut migrated = registry.clone();
    for task in &mut migrated.task {
        for member in &mut task.repo {
            let key = (task.name.clone(), member.name.clone());
            let write = supplied.remove(&key).ok_or_else(|| {
                Error::new(format!(
                    "migration requires an explicit claim for {}/{}",
                    task.name, member.name
                ))
            })?;
            member.write = claim::normalize(&write)?;
            member.boundary = None;
        }
    }
    if let Some(((task, member), _)) = supplied.into_iter().next() {
        return Err(Error::new(format!(
            "migration claim names an unknown member {task}/{member}"
        )));
    }
    migrated.version = 2;
    migrated.validate()?;
    Ok(migrated)
}

struct Seat {
    owner: String,
    identity: std::path::PathBuf,
    write: Vec<String>,
    selected: bool,
}

fn conflicts(space: &Space, selected: &Domain, migrated: &Registry) -> Result<()> {
    let mut held = Vec::new();
    for domain in space.domains()? {
        held.extend(seats(&domain, selected, migrated)?);
    }
    for left in 0..held.len() {
        for right in left + 1..held.len() {
            conflict(&held[left], &held[right])?;
        }
    }
    Ok(())
}

fn seats(domain: &Domain, selected: &Domain, migrated: &Registry) -> Result<Vec<Seat>> {
    let selected = domain.name() == selected.name();
    let registry = if selected {
        migrated.clone()
    } else {
        domain.registry()?
    };
    let mut seats = Vec::new();
    for task in &registry.task {
        let resolved = domain.task(&task.name)?;
        for member in &task.repo {
            let source = resolved.source(&member.source)?;
            let write = if registry.version == 1 {
                vec![".".to_string()]
            } else {
                member.write.clone()
            };
            seats.push(Seat {
                owner: format!("{}/{}/{}", domain.name(), task.name, member.name),
                identity: git::at(&source).identity()?,
                write,
                selected,
            });
        }
    }
    Ok(seats)
}

fn conflict(left: &Seat, right: &Seat) -> Result<()> {
    if !left.selected && !right.selected {
        return Ok(());
    }
    if left.identity != right.identity || !claim::overlaps(&left.write, &right.write) {
        return Ok(());
    }
    Err(Error::new(format!(
        "migration claim overlaps active member: {} and {}",
        left.owner, right.owner
    )))
}
