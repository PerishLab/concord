mod acceptance;
mod activity;
mod artifact;
mod configuration;
mod forge;
mod input;
mod integration;
mod member;
mod occupancy;

use crate::args::{self, Command};
use crate::config::Config;
use crate::{output, skill};
use concord_core::{Estate, Result, Seat};
use serde_json::json;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Completion {
    Done,
    Pending,
}

impl Completion {
    pub(crate) fn code(self) -> i32 {
        match self {
            Self::Done => 0,
            Self::Pending => 3,
        }
    }
}

pub async fn run(cli: args::Cli) -> Result<Completion> {
    if let Command::Cookbook(args) = &cli.command {
        return output::cookbook::run(args.code.as_deref(), cli.json).map(|()| Completion::Done);
    }
    if let Command::Member(args) = &cli.command {
        member::supported(&args.command)?;
    }
    if let Command::Config(config) = &cli.command
        && matches!(config.command, args::Configure::Path)
    {
        output::value(
            json!(Config::path(cli.config.as_deref())?.display().to_string()),
            cli.json,
        );
        return Ok(Completion::Done);
    }
    let loaded = Config::load(
        cli.config.as_deref(),
        cli.root.as_deref(),
        cli.home.as_deref(),
        cli.releases.as_deref(),
    )?;
    let config = loaded.config();
    let command = match cli.command {
        Command::Issue(args)
            if matches!(args.command, crate::args::issue::Command::Close { .. }) =>
        {
            let crate::args::issue::Command::Close { command } = args.command else {
                unreachable!()
            };
            return forge::close::run(command, cli.json)
                .await
                .map(|()| Completion::Done);
        }
        Command::Acceptance(args) => {
            return acceptance::run(args.command, cli.json)
                .await
                .map(|()| Completion::Done);
        }
        Command::Project(args) => {
            return crate::project::run(args.command, cli.json, config.phase.as_ref())
                .await
                .map(|()| Completion::Done);
        }
        Command::Issue(args)
            if matches!(args.command, crate::args::issue::Command::Phase { .. }) =>
        {
            let crate::args::issue::Command::Phase { command } = args.command else {
                unreachable!()
            };
            return crate::phase::run(command, config.phase.as_ref(), cli.json)
                .await
                .map(|()| Completion::Done);
        }
        Command::Skill(skill) => {
            return skill::run(config, skill.command, cli.json).map(|()| Completion::Done);
        }
        Command::Config(args) => {
            return configuration::run(config, args.command, cli.json).map(|()| Completion::Done);
        }
        command => command,
    };
    let root = loaded.root()?;
    let seat = Seat::new(root.path());
    match command {
        Command::Issue(args) if matches!(args.command, crate::args::issue::Command::Bootstrap) => {
            bootstrap(&seat, cli.json).await.map(|()| Completion::Done)
        }
        command => {
            Dispatch {
                activity: activity::Run::new(cli.json),
                estate: seat
                    .open()
                    .await
                    .map_err(|error| loaded.absent(&root, error))?,
                json: cli.json,
                occupancy: occupancy::Run::new(cli.json),
                phase: config.phase.as_ref(),
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

struct Dispatch<'a> {
    phase: Option<&'a crate::phase::Binding>,
    activity: activity::Run,
    estate: Estate,
    json: bool,
    occupancy: occupancy::Run,
}

impl Dispatch<'_> {
    async fn run(&self, command: Command) -> Result<Completion> {
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
            Command::Issue(args::issue::Args {
                command: args::issue::Command::Delivery { command },
            }) => forge::delivery::run(&self.estate, command, self.json).await,
            command => self.execute(command).await.map(|()| Completion::Done),
        };
        if matches!(result, Ok(Completion::Done)) {
            self.occupancy.commit(&self.estate, intent, operation);
        }
        result
    }

    async fn execute(&self, command: Command) -> Result<()> {
        match command {
            Command::Acceptance(_)
            | Command::Project(_)
            | Command::Config(_)
            | Command::Cookbook(_)
            | Command::Skill(_) => {
                unreachable!("handled before estate open")
            }
            Command::Issue(args) => {
                forge::run(&self.estate, args.command, self.json, self.phase).await
            }
            Command::Integration(args) => {
                integration::run(&self.estate, args.command, self.json).await
            }
            Command::Member(args) => member::run(&self.estate, args.command, self.json).await,
            Command::Artifact(args) => artifact::run(&self.estate, args.command, self.json).await,
            Command::Audit(_) => {
                let report = self.estate.settled().await?;
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
