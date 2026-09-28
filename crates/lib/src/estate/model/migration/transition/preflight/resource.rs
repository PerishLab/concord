use super::super::{Artifact, Member};
use super::{Disposition, IssueDestination, Mapping, ResourceDisposition, TaskDisposition};
use crate::{Error, Result, Seat};
use std::collections::BTreeMap;
use std::path::Path;

pub struct Source<'a> {
    path: &'a Path,
    task: &'a str,
}

pub struct Set<'a> {
    kind: &'static str,
    dispositions: &'a [ResourceDisposition],
    expected: BTreeMap<(&'a str, &'a str), Source<'a>>,
}

impl<'a> Set<'a> {
    pub fn members(dispositions: &'a [ResourceDisposition], members: &'a [Member]) -> Self {
        Self {
            kind: "member",
            dispositions,
            expected: members
                .iter()
                .map(|member| {
                    (
                        (member.task.as_str(), member.name.as_str()),
                        Source {
                            path: &member.path,
                            task: &member.task,
                        },
                    )
                })
                .collect(),
        }
    }

    pub fn artifacts(dispositions: &'a [ResourceDisposition], artifacts: &'a [Artifact]) -> Self {
        Self {
            kind: "artifact",
            dispositions,
            expected: artifacts
                .iter()
                .map(|artifact| {
                    (
                        (artifact.task.as_str(), artifact.name.as_str()),
                        Source {
                            path: &artifact.path,
                            task: &artifact.task,
                        },
                    )
                })
                .collect(),
        }
    }
}

pub fn check(
    seat: &Seat,
    set: &Set<'_>,
    tasks: &BTreeMap<String, TaskDisposition>,
) -> Result<Vec<Mapping>> {
    let mut declared = BTreeMap::new();
    for disposition in set.dispositions {
        let key = (disposition.task.as_str(), disposition.name.as_str());
        if declared.insert(key, disposition).is_some() {
            return Err(Error::typed(
                "concord.transition.resource_duplicate",
                format!(
                    "{} has multiple dispositions: {}/{}",
                    set.kind, key.0, key.1
                ),
            ));
        }
        if !set.expected.contains_key(&key) {
            return Err(Error::typed(
                "concord.transition.resource_extraneous",
                format!(
                    "disposition names an unknown {}: {}/{}",
                    set.kind, key.0, key.1
                ),
            ));
        }
    }
    let mut mappings = Vec::new();
    for (key, source) in &set.expected {
        let disposition = declared.get(key).ok_or_else(|| {
            Error::typed(
                "concord.transition.resource_missing",
                format!("{} has no disposition: {}/{}", set.kind, key.0, key.1),
            )
        })?;
        let issue = issue(source.task, tasks, set.kind, key)?;
        if disposition.node != issue.node {
            return Err(Error::typed(
                "concord.transition.resource_node",
                format!(
                    "{} target node disagrees with its Task: {}/{}",
                    set.kind, key.0, key.1
                ),
            ));
        }
        let target = seat
            .space
            .join(".issues")
            .join(&issue.node)
            .join(if set.kind == "member" {
                "members"
            } else {
                "artifacts"
            })
            .join(key.1);
        vacant(&target, &disposition.path, set.kind, key)?;
        mappings.push(Mapping {
            task: key.0.to_string(),
            name: key.1.to_string(),
            node: issue.node.clone(),
            source: source.path.to_path_buf(),
            target,
        });
    }
    mappings.sort_by_key(|mapping| (mapping.task.clone(), mapping.name.clone()));
    Ok(mappings)
}

fn issue<'a>(
    task: &str,
    tasks: &'a BTreeMap<String, TaskDisposition>,
    kind: &str,
    key: &(&str, &str),
) -> Result<&'a IssueDestination> {
    match &tasks
        .get(task)
        .ok_or_else(|| Error::typed("concord.transition.disposition_missing", task))?
        .disposition
    {
        Disposition::Issue { issue } => Ok(issue),
        Disposition::ArchiveOnly => Err(Error::typed(
            "concord.transition.resource_archive",
            format!(
                "live {kind} cannot target an archive-only Task: {}/{}",
                key.0, key.1
            ),
        )),
    }
}

fn vacant(target: &Path, declared: &Path, kind: &str, key: &(&str, &str)) -> Result<()> {
    if declared != target {
        return Err(Error::typed(
            "concord.transition.resource_path",
            format!("{kind} target path is not canonical: {}/{}", key.0, key.1),
        ));
    }
    match std::fs::symlink_metadata(target) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Ok(_) => Err(Error::typed(
            "concord.transition.target_occupied",
            format!("{kind} target path is occupied: {}", target.display()),
        )),
        Err(error) => Err(error.into()),
    }
}
