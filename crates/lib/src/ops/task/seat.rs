use crate::git;
use crate::{Result, TaskRef};
use std::path::{Path, PathBuf};

type Repair = (PathBuf, PathBuf, PathBuf);

pub(super) fn repairs(task: &TaskRef, to: &Path) -> Result<Vec<Repair>> {
    task.task()
        .repo
        .iter()
        .map(|member| {
            Ok((
                task.source(&member.source)?,
                task.member_path(&member.name),
                to.join(&member.name),
            ))
        })
        .collect()
}

pub(super) fn apply(repairs: &[Repair], forward: bool) -> Result<()> {
    for (source, from, to) in repairs {
        git::at(source).repair(if forward { to } else { from })?;
    }
    Ok(())
}
