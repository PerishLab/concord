use crate::config::Config;
use crate::output;
use clap::Subcommand;
use concord_core::{Error, Result};
use plumb::skill::{Action, Ask, Kit};
use std::path::PathBuf;

#[derive(clap::Args)]
pub struct Args {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    #[command(about = "Install the Concord skill into detected agent directories")]
    Install {
        #[arg(long, default_value = "stable")]
        channel: String,
        #[arg(long)]
        version: Option<String>,
        #[arg(long)]
        path: Option<PathBuf>,
        #[arg(long)]
        force: bool,
    },
    #[command(about = "Upgrade all managed Concord skill installations")]
    Upgrade {
        #[arg(long, default_value = "stable")]
        channel: String,
        #[arg(long)]
        version: Option<String>,
        #[arg(long)]
        #[arg(long = "dry-run")]
        dry: bool,
    },
    #[command(about = "Compare managed Concord skills with a selected release")]
    Status {
        #[arg(long, default_value = "stable")]
        channel: String,
        #[arg(long)]
        version: Option<String>,
    },
    #[command(about = "Stage one exact non-stable Concord skill outside managed state")]
    Stage {
        #[arg(long)]
        channel: String,
        #[arg(long)]
        version: String,
        #[arg(long)]
        path: PathBuf,
    },
    #[command(about = "List managed Concord skill installations")]
    List,
    #[command(about = "Remove only managed Concord skill installations")]
    Uninstall,
}

impl Command {
    pub(crate) fn name(&self) -> &'static str {
        match self {
            Self::Install { .. } => "skill.install",
            Self::Upgrade { .. } => "skill.upgrade",
            Self::Status { .. } => "skill.status",
            Self::Stage { .. } => "skill.stage",
            Self::List => "skill.list",
            Self::Uninstall => "skill.uninstall",
        }
    }
}

pub fn run(config: &Config, command: Command, output: bool) -> Result<()> {
    let kit = kit(config)?;
    match command {
        Command::Install {
            channel,
            version,
            path,
            force,
        } => {
            let done = kit
                .install(&Ask {
                    channel,
                    version,
                    path,
                    force,
                })
                .map_err(failure)?;
            output::done("installed", &done, output)?;
            changed(&done)
        }
        Command::Upgrade {
            channel,
            version,
            dry,
        } => {
            let ask = Ask {
                channel,
                version,
                ..Ask::default()
            };
            if dry {
                let report = kit.status(&ask).map_err(failure)?;
                output::report("upgrade_dry_run", &report, output)?;
                actionable(&report)
            } else {
                let done = kit.upgrade(&ask).map_err(failure)?;
                output::done("upgraded", &done, output)?;
                movable(&done)
            }
        }
        Command::Status { channel, version } => {
            let report = kit
                .status(&Ask {
                    channel,
                    version,
                    ..Ask::default()
                })
                .map_err(failure)?;
            output::report("status", &report, output)
        }
        Command::Stage {
            channel,
            version,
            path,
        } => {
            let done = kit
                .stage(&Ask {
                    channel,
                    version: Some(version),
                    path: Some(path),
                    ..Ask::default()
                })
                .map_err(failure)?;
            output::done("staged", &done, output)?;
            changed(&done)
        }
        Command::List => {
            let records = kit.list().map_err(failure)?;
            output::records(&records, output)
        }
        Command::Uninstall => {
            let done = kit.uninstall().map_err(failure)?;
            output::done("removed", &done, output)?;
            changed(&done)
        }
    }
}

fn kit(config: &Config) -> Result<Kit> {
    let home = plumb::config::home()
        .ok_or_else(|| Error::new("platform home is unavailable for agent discovery"))?;
    Ok(Kit {
        name: "concord".to_string(),
        home,
        state: config.home.join("state/skills.json"),
        url: config.releases.clone(),
    })
}

fn changed(done: &plumb::skill::Done) -> Result<()> {
    if done.kept.is_empty() {
        return Err(Error::new("no managed skill installation changed"));
    }
    Ok(())
}

fn movable(done: &plumb::skill::Done) -> Result<()> {
    if done.kept.is_empty() && (done.same.is_empty() || !done.left.is_empty()) {
        return Err(Error::new("no managed skill installation can move"));
    }
    Ok(())
}

fn actionable(report: &plumb::skill::Report) -> Result<()> {
    if report.seats.is_empty()
        || report
            .seats
            .iter()
            .any(|status| status.action == Action::Refuse)
    {
        return Err(Error::new("skill upgrade plan is not actionable"));
    }
    Ok(())
}

fn failure(error: plumb::skill::Error) -> Error {
    Error::new(error.to_string())
}
