mod activity;
mod artifact;
mod configuration;
mod forge;
mod input;
mod member;
mod occupancy;

use crate::args::{self, Command};
use crate::config::Config;
use crate::{output, skill};
use concord_core::{Estate, Result, Seat};
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
        Command::Issue(args) if matches!(args.command, crate::args::issue::Command::Bootstrap) => {
            bootstrap(&seat, cli.json).await
        }
        command => {
            Dispatch {
                activity: activity::Run::new(cli.json),
                estate: seat.open().await?,
                json: cli.json,
                occupancy: occupancy::Run::new(cli.json),
            }
            .run(command)
            .await
        }
    }
}

async fn bootstrap(seat: &Seat, output: bool) -> Result<()> {
    drop(seat.bootstrap().await?);
    emit(json!({"estate": {"kind": "issue"}}), output)
}

struct Dispatch {
    activity: activity::Run,
    estate: Estate,
    json: bool,
    occupancy: occupancy::Run,
}

impl Dispatch {
    async fn run(&self, command: Command) -> Result<()> {
        let operation = command.name();
        let intent = self.occupancy.prepare(&self.estate, &command).await;
        if let Some(issues) = command.activity() {
            for issue in issues {
                self.activity
                    .touch(&self.estate, issue, command.name())
                    .await;
            }
        }
        let result = match command {
            Command::Config(_) | Command::Skill(_) => {
                unreachable!("handled before estate open")
            }
            Command::Issue(args) => forge::run(&self.estate, args.command, self.json).await,
            Command::Member(args) => member::run(&self.estate, args.command, self.json).await,
            Command::Artifact(args) => artifact::run(&self.estate, args.command, self.json).await,
            Command::Audit(_) => {
                let report = self.estate.inspect().await?;
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
        };
        if result.is_ok() {
            self.occupancy.commit(&self.estate, intent, operation);
        }
        result
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
