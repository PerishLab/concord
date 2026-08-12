use super::*;

struct Held {
    owner: String,
    path: std::path::PathBuf,
    identity: std::path::PathBuf,
    write: Vec<String>,
}

pub(super) fn conflicts(space: &Space, audit: &mut Audit) -> Result<()> {
    let mut held = Vec::new();
    for domain in space.domains()? {
        held.extend(members(&domain)?);
    }
    for left in 0..held.len() {
        for right in left + 1..held.len() {
            conflict(audit, &held[left], &held[right]);
        }
    }
    Ok(())
}

fn members(domain: &Domain) -> Result<Vec<Held>> {
    let registry = domain.registry()?;
    let mut held = Vec::new();
    for task in &registry.task {
        let resolved = domain.task(&task.name)?;
        for member in &task.repo {
            let source = match resolved.source(&member.source) {
                Ok(source) => source,
                Err(_) => continue,
            };
            let identity = match git::at(&source).identity() {
                Ok(identity) => identity,
                Err(_) => continue,
            };
            let write = if registry.version == 1 {
                vec![".".to_string()]
            } else {
                member.write.clone()
            };
            held.push(Held {
                owner: format!("{}/{}/{}", domain.name(), task.name, member.name),
                path: resolved.member(&member.name),
                identity,
                write,
            });
        }
    }
    Ok(held)
}

fn conflict(audit: &mut Audit, left: &Held, right: &Held) {
    if left.identity != right.identity || !crate::claim::overlaps(&left.write, &right.write) {
        return;
    }
    audit.fault(
        "claim",
        &right.path,
        format!("{} overlaps {}", right.owner, left.owner),
    );
}
