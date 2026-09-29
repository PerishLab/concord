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
    Member(&'a str),
    MemberSurfaces(&'a str, Option<&'a [String]>),
    Addition(&'a str, &'a [String]),
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
            Seed::Member(issue) => {
                let anchor = estate.issue(&Coordinate::parse(issue)?).await?;
                vec![Subject::IssueMember { node: anchor.node }]
            }
            Seed::MemberSurfaces(issue, additions) => {
                let coordinate = Coordinate::parse(issue)?;
                let held = estate.issue_member(&coordinate).await?;
                let mut claims = held.claims;
                claims.extend(additions.into_iter().flatten().cloned());
                let source = estate.issue_source_path(&coordinate).await?;
                estate.write_surfaces(&source, &claims)?
            }
            Seed::Addition(issue, claims) => {
                let coordinate = Coordinate::parse(issue)?;
                let integration = estate.integration(&coordinate).await?;
                estate.write_surfaces(Path::new(&integration.path), claims)?
            }
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
        args::Command::Integration(_) => None,
        args::Command::Config(_)
        | args::Command::Cookbook(_)
        | args::Command::Audit(_)
        | args::Command::Skill(_) => None,
    }
}

fn issue(command: &args::issue::Command) -> Option<Vec<Seed<'_>>> {
    match command {
        args::issue::Command::Delivery {
            command: args::issue::Delivery::Land { issue, .. },
        } => Some(vec![
            Seed::Issue(issue),
            Seed::Member(issue),
            Seed::MemberSurfaces(issue, None),
        ]),
        _ => None,
    }
}

fn member(command: &args::member::Command) -> Option<Vec<Seed<'_>>> {
    use args::member::Command;
    let issue = match command {
        Command::Start { issue, .. }
        | Command::Claim { issue, .. }
        | Command::Narrow { issue, .. }
        | Command::Prove { issue, .. }
        | Command::Release { issue, .. }
        | Command::Complete { issue, .. }
        | Command::Retire { issue, .. } => issue.as_str(),
        Command::Reference { command } => command.issue(),
        Command::List { .. } | Command::Status { .. } | Command::Landing { .. } => return None,
    };
    let mut seeds = vec![Seed::Issue(issue), Seed::Member(issue)];
    match command {
        Command::Start { claim, .. } => seeds.push(Seed::Addition(issue, claim)),
        Command::Claim { claim, .. } | Command::Narrow { claim, .. } => {
            seeds.push(Seed::MemberSurfaces(issue, Some(claim)))
        }
        _ => seeds.push(Seed::MemberSurfaces(issue, None)),
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
