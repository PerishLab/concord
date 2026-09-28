use crate::{args, output};
use concord_core::activity::Operator;
use concord_core::occupancy::Subject;
use concord_core::{Coordinate, Estate, Result};
use std::path::Path;

pub(crate) struct Run {
    operator: Option<Operator>,
    output: bool,
}

pub(crate) enum Intent {
    None,
    Ready(Vec<Subject>),
    Failed(concord_core::Error),
}

enum Seed<'a> {
    Issue(&'a str),
    Member(&'a str, &'a str),
    MemberSurfaces(&'a str, &'a str, Option<&'a [String]>),
    Surfaces(&'a Path, &'a [String]),
}

impl Run {
    pub(crate) fn new(output: bool) -> Self {
        Self {
            operator: Operator::detect(),
            output,
        }
    }

    pub(crate) async fn prepare(&self, estate: &Estate, command: &args::Command) -> Intent {
        let Some(seeds) = seeds(command) else {
            return Intent::None;
        };
        match resolve(estate, seeds).await {
            Ok(subjects) => Intent::Ready(subjects),
            Err(error) => Intent::Failed(error),
        }
    }

    pub(crate) fn commit(&self, estate: &Estate, intent: Intent, operation: &str) {
        match intent {
            Intent::None => {}
            Intent::Failed(error) => output::blind(&error, self.output),
            Intent::Ready(subjects) => self.write(estate, operation, &subjects),
        }
    }

    fn write(&self, estate: &Estate, operation: &str, subjects: &[Subject]) {
        let Some(operator) = self.operator.as_ref() else {
            return;
        };
        match estate.occupy(operator, operation, subjects) {
            Ok(occupancy) => output::occupancy(&occupancy, self.output),
            Err(error) => output::blind(&error, self.output),
        }
    }
}

async fn resolve(estate: &Estate, seeds: Vec<Seed<'_>>) -> Result<Vec<Subject>> {
    let mut subjects = Vec::new();
    for seed in seeds {
        let found = match seed {
            Seed::Issue(issue) => {
                let anchor = estate.issue(&Coordinate::parse(issue)?).await?;
                vec![Subject::Issue { node: anchor.node }]
            }
            Seed::Member(issue, member) => {
                let anchor = estate.issue(&Coordinate::parse(issue)?).await?;
                vec![Subject::IssueMember {
                    node: anchor.node,
                    member: member.to_string(),
                }]
            }
            Seed::MemberSurfaces(issue, member, additions) => {
                let coordinate = Coordinate::parse(issue)?;
                let held = estate.issue_member(&coordinate, member).await?;
                let mut claims = held.claims;
                claims.extend(additions.into_iter().flatten().cloned());
                let source = estate.issue_source_path(&coordinate, member).await?;
                estate.write_surfaces(&source, &claims)?
            }
            Seed::Surfaces(source, claims) => estate.write_surfaces(source, claims)?,
        };
        subjects.extend(found);
    }
    subjects.sort();
    subjects.dedup();
    Ok(subjects)
}

fn seeds(command: &args::Command) -> Option<Vec<Seed<'_>>> {
    match command {
        args::Command::Member(args) => member(&args.command),
        args::Command::Issue(args) => issue(&args.command),
        args::Command::Artifact(args) => artifact(&args.command),
        args::Command::Config(_) | args::Command::Audit(_) | args::Command::Skill(_) => None,
    }
}

fn issue(command: &args::issue::Command) -> Option<Vec<Seed<'_>>> {
    match command {
        args::issue::Command::Delivery {
            command: args::issue::Delivery::Land { issue, member, .. },
        } => Some(vec![
            Seed::Issue(issue),
            Seed::Member(issue, member),
            Seed::MemberSurfaces(issue, member, None),
        ]),
        _ => None,
    }
}

fn member(command: &args::member::Command) -> Option<Vec<Seed<'_>>> {
    use args::member::Command;
    let (issue, member) = match command {
        Command::Attach { issue, name, .. } => (issue.as_str(), name),
        Command::Claim { issue, member, .. }
        | Command::Narrow { issue, member, .. }
        | Command::Prove { issue, member, .. }
        | Command::Release { issue, member, .. }
        | Command::Retire { issue, member, .. } => (issue.as_str(), member),
        Command::Reference { command } => (
            command.issue(),
            match command {
                args::member::Reference::Set { member, .. }
                | args::member::Reference::Remove { member, .. } => member,
            },
        ),
        Command::List { .. } | Command::Status { .. } | Command::Landing { .. } => return None,
    };
    let mut seeds = vec![Seed::Issue(issue), Seed::Member(issue, member)];
    match command {
        Command::Attach { source, claim, .. } => seeds.push(Seed::Surfaces(source, claim)),
        Command::Claim { claim, .. } | Command::Narrow { claim, .. } => {
            seeds.push(Seed::MemberSurfaces(issue, member, Some(claim)))
        }
        _ => seeds.push(Seed::MemberSurfaces(issue, member, None)),
    }
    Some(seeds)
}

fn artifact(command: &args::artifact::Command) -> Option<Vec<Seed<'_>>> {
    match command {
        args::artifact::Command::Import { issue, .. }
        | args::artifact::Command::Remove { issue, .. } => Some(vec![Seed::Issue(issue)]),
        _ => None,
    }
}
