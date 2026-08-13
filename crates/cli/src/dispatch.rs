mod activity;
mod artifact;
mod configuration;
mod domain;
mod graph;
mod input;
mod member;
mod task;

use crate::args::{self, Command};
use crate::config::Config;
use crate::{output, skill};
use concord_core::{Estate, Result, Seat, Settle};
use serde_json::json;

pub async fn run(cli: args::Cli) -> Result<()> {
    if let Command::Config(config) = &cli.command
        && matches!(config.command, args::Configure::Path)
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
        Command::Config(args) => {
            return configuration::run(&config, args.command, cli.json);
        }
        command => command,
    };
    let root = config.root()?;
    let seat = Seat::new(root.path());
    match command {
        Command::Domain(args)
            if matches!(args.command, crate::args::domain::Command::Bootstrap { .. }) =>
        {
            let crate::args::domain::Command::Bootstrap { name } = args.command else {
                unreachable!()
            };
            let estate = seat.bootstrap().await?;
            let key = estate.manage(&name).await?;
            emit(domain::value(key, name), cli.json)
        }
        command => {
            Dispatch {
                activity: activity::Run::new(cli.json),
                estate: seat.open().await?,
                json: cli.json,
            }
            .run(command)
            .await
        }
    }
}

struct Dispatch {
    activity: activity::Run,
    estate: Estate,
    json: bool,
}

impl Dispatch {
    async fn run(&self, command: Command) -> Result<()> {
        if let Some((operation, tasks)) = command.activity() {
            for task in tasks {
                self.activity.touch(&self.estate, task, operation).await;
            }
        }
        match command {
            Command::Config(_) | Command::Skill(_) => {
                unreachable!("handled before estate open")
            }
            Command::Domain(args) => domain::run(&self.estate, args.command, self.json).await,
            Command::Task(args) => {
                task::run(&self.estate, args.command, &self.activity, self.json).await
            }
            Command::Phase(args) => self.phase(args.command).await,
            Command::Member(args) => member::run(&self.estate, args.command, self.json).await,
            Command::Artifact(args) => artifact::run(&self.estate, args.command, self.json).await,
            Command::Graph(args) => graph::run(&self.estate, args.command, self.json).await,
            Command::Audit(args) => {
                let report = self
                    .estate
                    .inspect(args.task.as_deref(), args.domain.as_deref())
                    .await?;
                let agrees = report.agrees();
                emit(json!({"agreement": report}), self.json)?;
                if agrees {
                    Ok(())
                } else {
                    Err(concord_core::Error::typed(
                        "concord.audit.faults",
                        "estate audit found agreement faults",
                    ))
                }
            }
        }
    }

    async fn phase(&self, command: args::phase::Command) -> Result<()> {
        match command {
            args::phase::Command::List { task } => emit(
                json!({"task": task, "phases": self.estate.phases(&task).await?}),
                self.json,
            ),
            args::phase::Command::Settle { input: path } => {
                let settle: Settle = input::read(&path)?;
                self.activity
                    .touch(&self.estate, &settle.task, "phase.settle")
                    .await;
                emit(
                    json!({"settlement": self.estate.settle(&settle).await?}),
                    self.json,
                )
            }
        }
    }
}

pub(super) fn emit(body: serde_json::Value, json: bool) -> Result<()> {
    let mut body = body;
    let object = body
        .as_object_mut()
        .expect("Concord CLI protocol body must be an object");
    object.insert("version".to_string(), json!(1));
    output::value(body, json);
    Ok(())
}

pub(super) fn explicit(apply: bool, operation: &str) -> Result<()> {
    if apply {
        return Ok(());
    }
    Err(concord_core::Error::typed(
        "concord.apply.required",
        format!("{operation} requires explicit --apply"),
    ))
}
