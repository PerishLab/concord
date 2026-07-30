mod artifact;
mod mutation;

use crate::args::{
    AuditArgs, Cli, Command, ConfigCommand, DomainCommand, MemberCommand, PermissionCommand,
    RepoCommand, TaskCommand,
};
use crate::config::Config;
use crate::output;
use crate::skill;
use concord_core::{Add, Result, Space};
use serde_json::json;

pub(crate) use mutation::{create, guarded};

pub fn run(cli: Cli) -> Result<()> {
    if let Command::Config(config) = &cli.command
        && matches!(config.command, ConfigCommand::Path)
    {
        output::value(
            json!(Config::path(cli.config.as_deref())?.display().to_string()),
            cli.json,
        );
        return Ok(());
    }
    let config = Config::load(
        cli.config.as_deref(),
        cli.root.as_deref(),
        cli.home.as_deref(),
        cli.releases.as_deref(),
    )?;
    let command = match cli.command {
        Command::Skill(skill) => return skill::run(&config, skill.command, cli.json),
        Command::Config(config_command) => {
            return configure(&config, config_command.command, cli.json);
        }
        command => command,
    };
    Dispatch {
        space: Space::new(config.root()?),
        json: cli.json,
    }
    .run(command)
}

fn configure(config: &Config, command: ConfigCommand, json_output: bool) -> Result<()> {
    match command {
        ConfigCommand::Path => unreachable!("config path exits before loading the space"),
        ConfigCommand::Show => {
            output::value(
                json!({
                    "domain_space_root": config.domain_space_root.display().to_string(),
                    "home": config.home.display().to_string(),
                    "releases": config.releases,
                }),
                json_output,
            );
            Ok(())
        }
    }
}

struct Dispatch {
    space: Space,
    json: bool,
}

impl Dispatch {
    fn run(&self, command: Command) -> Result<()> {
        match command {
            Command::Config(_) | Command::Skill(_) => unreachable!("handled before task config"),
            Command::Domain(domain) => self.domain(domain.command),
            Command::Repo(repo) => self.repo(repo.command),
            Command::Task(task) => self.task(task.command),
            Command::Member(member) => self.member(member.command),
            Command::Memory(memory) => {
                artifact::memory_command(&self.space, memory.command, self.json)
            }
            Command::Resource(resource) => {
                artifact::resource_command(&self.space, resource.command, self.json)
            }
            Command::Permissions(permission) => self.permission(permission.command),
            Command::Audit(audit) => self.audit(audit),
        }
    }

    fn domain(&self, command: DomainCommand) -> Result<()> {
        match command {
            DomainCommand::List => {
                let values = self
                    .space
                    .domains()?
                    .into_iter()
                    .map(|domain| json!(domain.name()))
                    .collect();
                output::value(serde_json::Value::Array(values), self.json);
                Ok(())
            }
            DomainCommand::Init { name, dry_run } => create(
                self.space.domain_init(&name, false)?,
                dry_run,
                self.json,
                || self.space.domain_init(&name, true),
            ),
        }
    }

    fn repo(&self, command: RepoCommand) -> Result<()> {
        match command {
            RepoCommand::Annotate {
                domain,
                name,
                note,
                dry_run,
            } => create(
                self.space
                    .repo_annotate(&domain, &name, note.as_deref(), false)?,
                dry_run,
                self.json,
                || {
                    self.space
                        .repo_annotate(&domain, &name, note.as_deref(), true)
                },
            ),
        }
    }

    fn task(&self, command: TaskCommand) -> Result<()> {
        match command {
            TaskCommand::List { domain } => {
                let values = self.tasks(domain.as_deref())?;
                output::value(serde_json::Value::Array(values), self.json);
                Ok(())
            }
            TaskCommand::Show { task } => {
                let task = self.space.resolve(&task)?;
                output::value(
                    json!({
                        "identity": task.identity(),
                        "path": task.path().display().to_string(),
                        "task": task.task()
                    }),
                    self.json,
                );
                Ok(())
            }
            TaskCommand::Start { task, dry_run } => create(
                self.space.task_start(&task, false)?,
                dry_run,
                self.json,
                || self.space.task_start(&task, true),
            ),
            TaskCommand::Rename { task, name, apply } => guarded(
                self.space.task_rename(&task, &name, false)?,
                apply,
                self.json,
                || self.space.task_rename(&task, &name, true),
            ),
            TaskCommand::Rehome {
                task,
                domain,
                apply,
            } => guarded(
                self.space.task_rehome(&task, &domain, false)?,
                apply,
                self.json,
                || self.space.task_rehome(&task, &domain, true),
            ),
            TaskCommand::Finish { task, apply } => guarded(
                self.space.task_finish(&task, false)?,
                apply,
                self.json,
                || self.space.task_finish(&task, true),
            ),
        }
    }

    fn tasks(&self, selected: Option<&str>) -> Result<Vec<serde_json::Value>> {
        let domains = match selected {
            Some(name) => vec![self.space.domain(name)?],
            None => self.space.domains()?,
        };
        let mut values = Vec::new();
        for domain in domains {
            let name = domain.name().to_string();
            values.extend(domain.registry()?.task.into_iter().map(|task| {
                json!({
                    "identity": format!("{}/{}", name, task.name),
                    "task": task
                })
            }));
        }
        Ok(values)
    }

    fn member(&self, command: MemberCommand) -> Result<()> {
        match command {
            MemberCommand::Add {
                task,
                source,
                name,
                branch,
                orphan,
                dry_run,
            } => {
                let name = match name {
                    Some(name) => name,
                    None => source
                        .file_name()
                        .and_then(|value| value.to_str())
                        .ok_or_else(|| concord_core::Error::new("source has no utf8 filename"))?
                        .to_string(),
                };
                let request = || Add {
                    task: &task,
                    name: &name,
                    source: &source,
                    branch: branch.as_deref(),
                    orphan,
                };
                create(
                    self.space.member_add(request(), false)?,
                    dry_run,
                    self.json,
                    || self.space.member_add(request(), true),
                )
            }
            MemberCommand::Preflight { task } => {
                output::preflight(&self.space.resolve(&task)?.preflight()?, self.json)
            }
            MemberCommand::RemoveLanded { task, name, apply } => guarded(
                self.space.member_remove(&task, &name, false)?,
                apply,
                self.json,
                || self.space.member_remove(&task, &name, true),
            ),
        }
    }

    fn permission(&self, command: PermissionCommand) -> Result<()> {
        match command {
            PermissionCommand::Normalize { task, apply } => guarded(
                self.space.normalize(&task, false)?,
                apply,
                self.json,
                || self.space.normalize(&task, true),
            ),
        }
    }

    fn audit(&self, args: AuditArgs) -> Result<()> {
        let audit = if args.space {
            if args.task.is_some() || args.domain.is_some() {
                return Err(concord_core::Error::new(
                    "--space cannot be combined with a task or --domain",
                ));
            }
            self.space.audit()?
        } else if let Some(task) = args.task {
            if args.domain.is_some() {
                return Err(concord_core::Error::new(
                    "task cannot be combined with --domain",
                ));
            }
            self.space.resolve(&task)?.audit()?
        } else if let Some(domain) = args.domain {
            self.space.domain(&domain)?.audit()?
        } else {
            return Err(concord_core::Error::new(
                "audit requires a task, --domain, or --space",
            ));
        };
        output::audit(&audit, self.json)
    }
}
