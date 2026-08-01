mod artifact;
mod configuration;
mod input;
mod migration;
mod mutation;
mod resource;
mod task;

use crate::args::{
    AuditArgs, Cli, Command, ConfigCommand, DomainCommand, MemberCommand, PermissionCommand,
    RepoCommand,
};
use crate::config::Config;
use crate::observation;
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
            return configuration::run(&config, config_command.command, cli.json);
        }
        command => command,
    };
    Dispatch {
        space: Space::new(config.root()?),
        json: cli.json,
    }
    .run(command)
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
            Command::Task(task_args) => task::command(&self.space, task_args.command, self.json),
            Command::Member(member) => self.member(member.command),
            Command::Memory(memory) => {
                observation::start();
                artifact::memory_command(&self.space, memory.command, self.json)
            }
            Command::Resource(resource) => {
                resource::command(&self.space, resource.command, self.json)
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
            DomainCommand::Migrate {
                domain,
                claim,
                apply,
            } => {
                let claims = migration::claims(&claim)?;
                guarded(
                    self.space.domain_migrate(&domain, &claims, false)?,
                    apply,
                    self.json,
                    || self.space.domain_migrate(&domain, &claims, true),
                )
            }
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

    fn member(&self, command: MemberCommand) -> Result<()> {
        match command {
            MemberCommand::Add {
                task,
                source,
                name,
                branch,
                orphan,
                write,
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
                    write: &write,
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
            MemberCommand::Claim {
                task,
                name,
                write,
                dry_run,
            } => create(
                self.space.member_claim(&task, &name, &write, false)?,
                dry_run,
                self.json,
                || self.space.member_claim(&task, &name, &write, true),
            ),
            MemberCommand::Boundary { task, name } => {
                let value = serde_json::to_value(self.space.member_boundary(&task, &name)?)
                    .map_err(|error| {
                        concord_core::Error::new(format!("cannot encode boundary proof: {error}"))
                    })?;
                output::value(value, self.json);
                Ok(())
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
