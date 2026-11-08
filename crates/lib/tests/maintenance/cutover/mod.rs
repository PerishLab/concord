mod files;
mod journal;
#[cfg(all(test, any(windows, target_os = "linux")))]
mod member;
#[cfg(all(test, any(windows, target_os = "linux")))]
mod tests;
mod transition;

use concord_core::{Error, Result, Seat};
use serde::{Deserialize, Serialize};
use std::ffi::OsString;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Intent {
    schema: String,
    source: PathBuf,
    home: PathBuf,
    files: files::Inventory,
    artifacts: Option<files::Inventory>,
}

pub(super) struct Cutover {
    source: PathBuf,
    home: PathBuf,
}

pub async fn entry(args: Vec<OsString>) -> Result<()> {
    if args.len() != 4 || args[3] != "--stopped" {
        return Err(refuse(
            "usage: cutover <stage|activate|abort|finish> <absolute-legacy-root> <absolute-home> --stopped; stop every old/new writer, settle unknown deliveries and drain all Members first; see Concord #452",
        ));
    }
    let source = Path::new(&args[1]);
    let home = Path::new(&args[2]);
    if !source.is_absolute() || !home.is_absolute() {
        return Err(refuse(
            "source and home must be explicit absolute directories",
        ));
    }
    files::directory(source)?;
    files::directory(home)?;
    let source = source.canonicalize()?;
    let home = home.canonicalize()?;
    if source.starts_with(&home) || home.starts_with(&source) {
        return Err(refuse(
            "source and target home must be disjoint directories",
        ));
    }
    let command = args[0].to_str().ok_or_else(|| refuse("invalid command"))?;
    volume(&source, &home)?;
    let cutover = Cutover {
        source: source.clone(),
        home: home.clone(),
    };
    let guards = if source.join(".concord").try_exists()? {
        Some(cutover.guards()?)
    } else {
        None
    };
    if command == "finish" {
        if let Some(guards) = guards {
            Cutover::release(guards, Ok(()))?;
        }
        return cutover.finish().await;
    }
    let guards = guards.ok_or_else(|| refuse("missing legacy controls"))?;
    let result = match command {
        "stage" => cutover.stage().await,
        "activate" => cutover.activate().await,
        "abort" => cutover.abort(),
        _ => Err(refuse(
            "unknown operation; use stage, activate, abort or finish",
        )),
    };
    Cutover::release(guards, result)?;
    println!("{command} complete; no provider operation or Git payload move performed");
    Ok(())
}

async fn validate(home: &Path) -> Result<String> {
    let estate = Seat::new(home).open().await?;
    if !estate.issue_worktrees().await?.is_empty() {
        return Err(refuse(
            "live Members refuse offline cutover; restore the frozen source with abort, then drain through its owning version",
        ));
    }
    let report = estate.settled().await?;
    if !report.agrees() {
        return Err(refuse(format!(
            "target agreement refuses: {}",
            serde_json::to_string(&report).map_err(|error| refuse(error.to_string()))?
        )));
    }
    Ok(estate.identity().into())
}

fn refuse(message: impl Into<String>) -> Error {
    Error::typed("concord.cutover.refused", message)
}

#[cfg(target_os = "linux")]
fn volume(source: &Path, home: &Path) -> Result<()> {
    use std::os::unix::fs::MetadataExt;
    if source.metadata()?.dev() != home.metadata()?.dev() {
        return Err(refuse(
            "cross-volume cutover is unqualified; preserve source and arrange a separately verified migration",
        ));
    }
    Ok(())
}

#[cfg(windows)]
fn volume(source: &Path, home: &Path) -> Result<()> {
    if source.components().next() != home.components().next() {
        return Err(refuse(
            "cross-drive cutover is unqualified; preserve source and arrange a separately verified migration",
        ));
    }
    Ok(())
}

#[cfg(not(any(windows, target_os = "linux")))]
fn volume(_: &Path, _: &Path) -> Result<()> {
    Err(refuse(
        "this platform has no qualified offline cutover; preserve source",
    ))
}

#[cfg(all(test, not(any(windows, target_os = "linux"))))]
#[test]
fn unsupported() {
    assert!(volume(Path::new("/"), Path::new("/")).is_err());
}
