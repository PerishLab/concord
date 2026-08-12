use super::super::scan::Work;
use super::extra;
use super::mismatch;
use crate::{Result, Worktree};
use keel::Row;

pub(super) fn check(
    members: &[Worktree],
    additions: &[Row],
    identity: &str,
    work: &Work,
) -> Result<()> {
    for member in &work.task.repo {
        let found = members
            .iter()
            .find(|held| held.task == identity && held.name == member.name)
            .ok_or_else(|| mismatch(format!("missing Member {identity}/{}", member.name)))?;
        let wanted = (
            member.source.as_str(),
            member.branch(&work.task.name),
            member.write.as_slice(),
        );
        let held = (
            found.source.as_str(),
            found.branch.as_str(),
            found.claims.as_slice(),
        );
        if wanted != held {
            return Err(mismatch(format!(
                "Member differs: {identity}/{}",
                member.name
            )));
        }
        proof(identity, member, found)?;
        extra::check(additions, "member", found.key, &member.extra)?;
    }
    Ok(())
}

fn proof(identity: &str, wanted: &crate::Member, held: &Worktree) -> Result<()> {
    let same = match (&wanted.boundary, &held.proof) {
        (None, None) => true,
        (Some(wanted), Some(held)) => {
            let wanted = (
                wanted.schema.as_str(),
                wanted.plumb.as_str(),
                wanted.base.as_str(),
                wanted.head.as_str(),
                wanted.claim.as_str(),
            );
            let held = (
                held.schema.as_str(),
                held.plumb.as_str(),
                held.base.as_str(),
                held.head.as_str(),
                held.claim.as_str(),
            );
            wanted == held
        }
        _ => false,
    };
    if same {
        return Ok(());
    }
    Err(mismatch(format!(
        "Boundary differs: {identity}/{}",
        wanted.name
    )))
}
