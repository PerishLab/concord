mod control;
mod files;
mod git;
mod journal;
mod member;
mod payload;
mod receipt;
#[cfg(test)]
#[path = "tests/main.rs"]
mod tests;
mod transition;

use concord_core::{Error, Result, Seat};
use serde::{Deserialize, Serialize};
use std::ffi::OsString;
use std::path::{Path, PathBuf};

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Intent {
    schema: String,
    source: PathBuf,
    home: PathBuf,
    files: files::Inventory,
    payload: Option<payload::Payload>,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Ready {
    space: String,
    files: files::Inventory,
    members: Vec<member::Member>,
    git: plumb::config::Tool,
}

struct Relocate {
    source: PathBuf,
    home: PathBuf,
}

pub async fn entry(args: Vec<OsString>) -> Result<()> {
    if args.len() != 4 || args[3] != "--stopped" {
        return Err(refuse(
            "usage: relocate <stage|activate|abort|finish> <absolute-source> <absolute-home> --stopped; stop all writers and settle unknown deliveries first",
        ));
    }
    let source = Path::new(&args[1]);
    let home = Path::new(&args[2]);
    if !source.is_absolute() || !home.is_absolute() {
        return Err(refuse("explicit absolute source/home required"));
    }
    files::directory(source)?;
    files::directory(home)?;
    let task = Relocate {
        source: source.canonicalize()?,
        home: home.canonicalize()?,
    };
    use std::os::unix::fs::MetadataExt;
    if task.source.starts_with(&task.home)
        || task.home.starts_with(&task.source)
        || task.source.metadata()?.dev() != task.home.metadata()?.dev()
    {
        return Err(refuse(
            "disjoint source/home on the same Linux filesystem required",
        ));
    }
    let command = args[0]
        .to_str()
        .ok_or_else(|| refuse("invalid operation"))?;
    let held = journal::hold(&task.source)?;
    let result = match command {
        "stage" => task.stage().await,
        "activate" => task.activate().await,
        "abort" => task.abort(),
        "finish" => task.finish().await,
        _ => Err(refuse("unknown operation")),
    };
    journal::release(held, result)?;
    println!("{command} complete; no provider write, source commit or Issue retirement performed");
    Ok(())
}

fn refuse(message: impl Into<String>) -> Error {
    Error::typed("concord.relocate.refused", message)
}
