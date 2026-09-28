use crate::args;
use crate::output;
use concord_core::Coordinate;
use concord_core::activity::Operator;
use concord_core::occupancy::Subject;
use concord_core::{Estate, Result};
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
    Task(&'a str),
    Member(&'a str, &'a str),
    Graph(&'a str),
    Issue(&'a str),
    IssueMember(&'a str, &'a str),
    MemberSurfaces(&'a str, &'a str, Option<&'a [String]>),
    Surfaces(&'a Path, &'a [String]),
    Exact(Subject),
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

    pub(crate) async fn task(&self, estate: &Estate, task: &str, operation: &str) {
        match resolve(estate, vec![Seed::Task(task)]).await {
            Ok(subjects) => self.write(estate, operation, &subjects),
            Err(error) => output::blind(&error, self.output),
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
            Seed::Task(task) => vec![Subject::Task {
                task: estate.node(task).await?.identity(),
            }],
            Seed::Member(task, member) => vec![Subject::Member {
                task: estate.node(task).await?.identity(),
                member: member.to_string(),
            }],
            Seed::Graph(task) => vec![Subject::Graph {
                task: estate.node(task).await?.identity(),
            }],
            Seed::Issue(issue) => {
                let anchor = estate.issue(&Coordinate::parse(issue)?).await?;
                vec![Subject::Issue { node: anchor.node }]
            }
            Seed::IssueMember(issue, member) => {
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
            Seed::Exact(subject) => vec![subject],
        };
        subjects.extend(found);
    }
    subjects.sort();
    subjects.dedup();
    Ok(subjects)
}

fn seeds(command: &args::Command) -> Option<Vec<Seed<'_>>> {
    match command {
        args::Command::Task(args) => task(&args.command),
        args::Command::Phase(_) => None,
        args::Command::Member(args) => member(&args.command),
        args::Command::Issue(args) => issue(&args.command),
        args::Command::Artifact(args) => artifact(&args.command),
        args::Command::Config(_)
        | args::Command::Domain(_)
        | args::Command::Graph(_)
        | args::Command::Audit(_)
        | args::Command::Skill(_) => None,
    }
}

fn issue(command: &args::issue::Command) -> Option<Vec<Seed<'_>>> {
    match command {
        args::issue::Command::Delivery {
            command: args::issue::Delivery::Land { issue, member, .. },
        } => Some(vec![
            Seed::Issue(issue),
            Seed::IssueMember(issue, member),
            Seed::MemberSurfaces(issue, member, None),
        ]),
        _ => None,
    }
}

fn task(command: &args::task::Command) -> Option<Vec<Seed<'_>>> {
    use args::task::Command;
    match command {
        Command::Start { domain, name, .. } => Some(vec![Seed::Exact(Subject::Task {
            task: format!("{domain}/{name}"),
        })]),
        Command::Rename { task, .. }
        | Command::Rehome { task, .. }
        | Command::Reference {
            command:
                args::task::Reference::Set { task, .. } | args::task::Reference::Remove { task, .. },
        } => Some(vec![Seed::Task(task)]),
        Command::Finish { task, .. } => Some(vec![Seed::Task(task), Seed::Graph(task)]),
        Command::Dependency { command } => dependency(command),
        Command::List { .. }
        | Command::Brief { .. }
        | Command::Show { .. }
        | Command::Change { .. } => None,
    }
}

fn dependency(command: &args::task::Dependency) -> Option<Vec<Seed<'_>>> {
    use args::task::Dependency;
    match command {
        Dependency::Add {
            source,
            target,
            create: true,
            ..
        } => Some(vec![
            Seed::Graph(source),
            Seed::Exact(Subject::Graph {
                task: target.clone(),
            }),
        ]),
        Dependency::Add { source, target, .. }
        | Dependency::Set { source, target, .. }
        | Dependency::Remove { source, target, .. } => {
            Some(vec![Seed::Graph(source), Seed::Graph(target)])
        }
        Dependency::List { .. } => None,
    }
}

fn member(command: &args::member::Command) -> Option<Vec<Seed<'_>>> {
    use args::member::Command;
    let (task, member) = match command {
        Command::Attach {
            task, name: member, ..
        }
        | Command::Claim { task, member, .. }
        | Command::Narrow { task, member, .. }
        | Command::Prove { task, member, .. }
        | Command::Reference {
            command:
                args::member::Reference::Set { task, member, .. }
                | args::member::Reference::Remove { task, member, .. },
        }
        | Command::Release { task, member, .. }
        | Command::Retire { task, member, .. } => (task, member),
        Command::List { .. } | Command::Status { .. } | Command::Landing { .. } => return None,
    };
    if task.contains('#') {
        let mut seeds = vec![Seed::Issue(task), Seed::IssueMember(task, member)];
        match command {
            Command::Attach { source, claim, .. } => {
                seeds.push(Seed::Surfaces(source, claim));
            }
            Command::Claim { claim, .. } | Command::Narrow { claim, .. } => {
                seeds.push(Seed::MemberSurfaces(task, member, Some(claim)));
            }
            Command::Prove { .. }
            | Command::Reference { .. }
            | Command::Release { .. }
            | Command::Retire { .. } => {
                seeds.push(Seed::MemberSurfaces(task, member, None));
            }
            Command::List { .. } | Command::Status { .. } | Command::Landing { .. } => {}
        }
        return Some(seeds);
    }
    Some(vec![Seed::Task(task), Seed::Member(task, member)])
}

fn artifact(command: &args::artifact::Command) -> Option<Vec<Seed<'_>>> {
    use args::artifact::Command;
    match command {
        Command::Import { task, .. } | Command::Remove { task, .. } if task.contains('#') => {
            Some(vec![Seed::Issue(task)])
        }
        Command::Import { task, .. } | Command::Remove { task, .. } => Some(vec![Seed::Task(task)]),
        Command::List { .. } | Command::Show { .. } | Command::Preflight { .. } => None,
    }
}
