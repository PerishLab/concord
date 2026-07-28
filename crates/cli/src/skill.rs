use crate::config::Config;
use crate::output;
use clap::{Args, Subcommand};
use concord_core::{Error, Result};
use plumb::skill::{Action, Ask, Kit};
use std::path::PathBuf;

#[derive(Args)]
pub struct SkillArgs {
    #[command(subcommand)]
    pub command: SkillCommand,
}

#[derive(Subcommand)]
pub enum SkillCommand {
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
        dry_run: bool,
    },
    #[command(about = "Compare managed Concord skills with a selected release")]
    Status {
        #[arg(long, default_value = "stable")]
        channel: String,
        #[arg(long)]
        version: Option<String>,
    },
    #[command(about = "List managed Concord skill installations")]
    List,
    #[command(about = "Remove only managed Concord skill installations")]
    Uninstall,
}

pub fn run(config: &Config, command: SkillCommand, json_output: bool) -> Result<()> {
    let kit = kit(config)?;
    match command {
        SkillCommand::Install {
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
                .map_err(skill_error)?;
            output::skill_done("installed", &done, json_output)?;
            changed(&done)
        }
        SkillCommand::Upgrade {
            channel,
            version,
            dry_run,
        } => {
            let ask = Ask {
                channel,
                version,
                ..Ask::default()
            };
            if dry_run {
                let report = kit.status(&ask).map_err(skill_error)?;
                output::skill_report("upgrade_dry_run", &report, json_output)?;
                actionable(&report)
            } else {
                let done = kit.upgrade(&ask).map_err(skill_error)?;
                output::skill_done("upgraded", &done, json_output)?;
                changed_or_same(&done)
            }
        }
        SkillCommand::Status { channel, version } => {
            let report = kit
                .status(&Ask {
                    channel,
                    version,
                    ..Ask::default()
                })
                .map_err(skill_error)?;
            output::skill_report("status", &report, json_output)
        }
        SkillCommand::List => {
            let records = kit.list().map_err(skill_error)?;
            output::skill_records(&records, json_output)
        }
        SkillCommand::Uninstall => {
            let done = kit.uninstall().map_err(skill_error)?;
            output::skill_done("removed", &done, json_output)?;
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

fn changed_or_same(done: &plumb::skill::Done) -> Result<()> {
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

fn skill_error(error: plumb::skill::Error) -> Error {
    Error::new(error.to_string())
}
