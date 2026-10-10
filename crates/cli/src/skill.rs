use crate::config::Config;
use crate::output;
use concord_core::{Error, Result};
use plumb::skill::command::Deed;
use plumb::skill::{Action, Ask, Kit};

#[derive(clap::Args)]
pub struct Args {
    #[command(subcommand)]
    pub command: Deed,
}

pub(crate) fn name(deed: &Deed) -> &'static str {
    match deed {
        Deed::Install { .. } => "skill.install",
        Deed::Upgrade { .. } => "skill.upgrade",
        Deed::Status { .. } => "skill.status",
        Deed::Stage { .. } => "skill.stage",
        Deed::List => "skill.list",
        Deed::Uninstall => "skill.uninstall",
    }
}

pub fn run(config: &Config, command: Deed, output: bool) -> Result<()> {
    let kit = kit(config)?;
    let kit = kit.depot(&config.depot, "concord", plumb::version!("CONCORD"));
    match command {
        Deed::Install {
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
        Deed::Upgrade {
            channel,
            version,
            dry_run,
            json,
        } => {
            let output = output || json;
            let ask = Ask {
                channel,
                version,
                ..Ask::default()
            };
            if dry_run {
                let report = kit.status(&ask).map_err(failure)?;
                output::report("upgrade_dry_run", &report, output)?;
                actionable(&report)
            } else {
                let done = kit.upgrade(&ask).map_err(failure)?;
                output::done("upgraded", &done, output)?;
                movable(&done)
            }
        }
        Deed::Status {
            channel,
            version,
            json,
        } => {
            let report = kit
                .status(&Ask {
                    channel,
                    version,
                    ..Ask::default()
                })
                .map_err(failure)?;
            output::report("status", &report, output || json)
        }
        Deed::Stage {
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
        Deed::List => {
            let records = kit.list().map_err(failure)?;
            output::records(&records, output)
        }
        Deed::Uninstall => {
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
