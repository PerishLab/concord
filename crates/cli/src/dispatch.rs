mod artifact;

use crate::args::{
    AuditArgs, Cli, Command, ConfigCommand, DomainCommand, MemberCommand, PermissionCommand,
    RepoCommand, TaskCommand,
};
use crate::config::Config;
use crate::output;
use crate::skill;
use concord_core::{Add, Plan, Result, Space};
use serde_json::json;

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
            return config_command_run(&config, config_command.command, cli.json);
        }
        command => command,
    };
    let space = Space::new(config.root()?);
    match command {
        Command::Config(_) | Command::Skill(_) => unreachable!("handled before task config"),
        Command::Domain(domain) => domain_command(&space, domain.command, cli.json),
        Command::Repo(repo) => repo_command(&space, repo.command, cli.json),
        Command::Task(task) => task_command(&space, task.command, cli.json),
        Command::Member(member) => member_command(&space, member.command, cli.json),
        Command::Memory(memory) => artifact::memory_command(&space, memory.command, cli.json),
        Command::Resource(resource) => {
            artifact::resource_command(&space, resource.command, cli.json)
        }
        Command::Permissions(permission) => {
            permission_command(&space, permission.command, cli.json)
        }
        Command::Audit(audit) => audit_command(&space, audit, cli.json),
    }
}

fn config_command_run(config: &Config, command: ConfigCommand, json_output: bool) -> Result<()> {
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

fn domain_command(space: &Space, command: DomainCommand, json_output: bool) -> Result<()> {
    match command {
        DomainCommand::List => {
            let values = space
                .domains()?
                .into_iter()
                .map(|domain| json!(domain.name()))
                .collect();
            output::value(serde_json::Value::Array(values), json_output);
            Ok(())
        }
        DomainCommand::Init { name, dry_run } => create(
            space.domain_init(&name, false)?,
            dry_run,
            json_output,
            || space.domain_init(&name, true),
        ),
    }
}

fn repo_command(space: &Space, command: RepoCommand, json_output: bool) -> Result<()> {
    match command {
        RepoCommand::Annotate {
            domain,
            name,
            note,
            dry_run,
        } => create(
            space.repo_annotate(&domain, &name, note.as_deref(), false)?,
            dry_run,
            json_output,
            || space.repo_annotate(&domain, &name, note.as_deref(), true),
        ),
    }
}

fn task_command(space: &Space, command: TaskCommand, json_output: bool) -> Result<()> {
    match command {
        TaskCommand::List { domain } => {
            let values = task_values(space, domain.as_deref())?;
            output::value(serde_json::Value::Array(values), json_output);
            Ok(())
        }
        TaskCommand::Show { task } => {
            let task = space.resolve(&task)?;
            output::value(
                json!({
                    "identity": task.identity(),
                    "path": task.path().display().to_string(),
                    "task": task.task()
                }),
                json_output,
            );
            Ok(())
        }
        TaskCommand::Start { task, dry_run } => create(
            space.task_start(&task, false)?,
            dry_run,
            json_output,
            || space.task_start(&task, true),
        ),
        TaskCommand::Rename { task, name, apply } => guarded(
            space.task_rename(&task, &name, false)?,
            apply,
            json_output,
            || space.task_rename(&task, &name, true),
        ),
        TaskCommand::Rehome {
            task,
            domain,
            apply,
        } => guarded(
            space.task_rehome(&task, &domain, false)?,
            apply,
            json_output,
            || space.task_rehome(&task, &domain, true),
        ),
        TaskCommand::Finish { task, apply } => {
            guarded(space.task_finish(&task, false)?, apply, json_output, || {
                space.task_finish(&task, true)
            })
        }
    }
}

fn task_values(space: &Space, selected: Option<&str>) -> Result<Vec<serde_json::Value>> {
    let domains = match selected {
        Some(name) => vec![space.domain(name)?],
        None => space.domains()?,
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

fn member_command(space: &Space, command: MemberCommand, json_output: bool) -> Result<()> {
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
                space.member_add(request(), false)?,
                dry_run,
                json_output,
                || space.member_add(request(), true),
            )
        }
        MemberCommand::Preflight { task } => {
            output::preflight(&space.resolve(&task)?.preflight()?, json_output)
        }
        MemberCommand::RemoveLanded { task, name, apply } => guarded(
            space.member_remove(&task, &name, false)?,
            apply,
            json_output,
            || space.member_remove(&task, &name, true),
        ),
    }
}

fn permission_command(space: &Space, command: PermissionCommand, json_output: bool) -> Result<()> {
    match command {
        PermissionCommand::Normalize { task, apply } => {
            guarded(space.normalize(&task, false)?, apply, json_output, || {
                space.normalize(&task, true)
            })
        }
    }
}

fn audit_command(space: &Space, args: AuditArgs, json_output: bool) -> Result<()> {
    let audit = if args.space {
        if args.task.is_some() || args.domain.is_some() {
            return Err(concord_core::Error::new(
                "--space cannot be combined with a task or --domain",
            ));
        }
        space.audit()?
    } else if let Some(task) = args.task {
        if args.domain.is_some() {
            return Err(concord_core::Error::new(
                "task cannot be combined with --domain",
            ));
        }
        space.resolve(&task)?.audit()?
    } else if let Some(domain) = args.domain {
        space.domain(&domain)?.audit()?
    } else {
        return Err(concord_core::Error::new(
            "audit requires a task, --domain, or --space",
        ));
    };
    output::audit(&audit, json_output)
}

fn create(
    plan: Plan,
    dry_run: bool,
    json_output: bool,
    apply: impl FnOnce() -> Result<Plan>,
) -> Result<()> {
    if dry_run {
        return output::plan(&plan, json_output);
    }
    if json_output {
        return output::plan(&apply()?, true);
    }
    output::plan(&plan, false)?;
    apply()?;
    output::applied(false);
    Ok(())
}

fn guarded(
    plan: Plan,
    apply_now: bool,
    json_output: bool,
    apply: impl FnOnce() -> Result<Plan>,
) -> Result<()> {
    if !apply_now {
        return output::plan(&plan, json_output);
    }
    if json_output {
        return output::plan(&apply()?, true);
    }
    output::plan(&plan, false)?;
    apply()?;
    output::applied(false);
    Ok(())
}
