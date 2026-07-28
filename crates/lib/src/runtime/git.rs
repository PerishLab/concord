use crate::{Error, Result};
use std::path::{Path, PathBuf};
use std::process::Command;

pub fn text(root: &Path, args: &[&str]) -> Result<String> {
    let output = command()
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .map_err(|error| Error::new(format!("cannot run git: {error}")))?;
    if !output.status.success() {
        let error = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(Error::new(format!(
            "git {} failed: {error}",
            args.join(" ")
        )));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

pub fn run(root: &Path, args: &[&str]) -> Result<()> {
    text(root, args).map(|_| ())
}

pub fn identity(root: &Path) -> Result<PathBuf> {
    let path = text(
        root,
        &["rev-parse", "--path-format=absolute", "--git-common-dir"],
    )?;
    PathBuf::from(path).canonicalize().map_err(Into::into)
}

pub fn branch(root: &Path) -> Result<String> {
    text(root, &["symbolic-ref", "--short", "HEAD"])
}

pub fn clean(root: &Path) -> Result<bool> {
    Ok(text(root, &["status", "--porcelain=v1", "--untracked-files=all"])?.is_empty())
}

pub fn reachable_from_head(member: &Path, source: &Path) -> Result<bool> {
    let head = text(member, &["rev-parse", "HEAD"])?;
    let status = command()
        .arg("-C")
        .arg(source)
        .args(["merge-base", "--is-ancestor", &head, "HEAD"])
        .status()
        .map_err(|error| Error::new(format!("cannot run git: {error}")))?;
    Ok(status.success())
}

pub fn registered(source: &Path, member: &Path) -> Result<bool> {
    let wanted = member.canonicalize()?;
    let list = text(source, &["worktree", "list", "--porcelain"])?;
    for line in list.lines() {
        if let Some(path) = line.strip_prefix("worktree ")
            && Path::new(path).canonicalize().ok().as_ref() == Some(&wanted)
        {
            return Ok(true);
        }
    }
    Ok(false)
}

pub fn repair(source: &Path, member: &Path) -> Result<()> {
    let member = member
        .to_str()
        .ok_or_else(|| Error::new("member path is not utf8"))?;
    run(source, &["worktree", "repair", member])
}

pub fn add(source: &Path, path: &Path, branch: &str, orphan: bool) -> Result<()> {
    let source = source
        .to_str()
        .ok_or_else(|| Error::new("source path is not utf8"))?;
    let path = path
        .to_str()
        .ok_or_else(|| Error::new("member path is not utf8"))?;
    let mut args = vec!["-C", source, "worktree", "add"];
    if orphan {
        args.extend(["--orphan", "-b", branch, path]);
    } else {
        args.extend(["-b", branch, path]);
    }
    let output = command().args(args).output()?;
    if !output.status.success() {
        return Err(Error::new(format!(
            "git worktree add failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    Ok(())
}

pub fn remove(source: &Path, path: &Path) -> Result<()> {
    let path = path
        .to_str()
        .ok_or_else(|| Error::new("member path is not utf8"))?;
    run(source, &["worktree", "remove", path])
}

fn command() -> Command {
    let mut command = Command::new("git");
    for name in [
        "GIT_ALTERNATE_OBJECT_DIRECTORIES",
        "GIT_COMMON_DIR",
        "GIT_DIR",
        "GIT_INDEX_FILE",
        "GIT_OBJECT_DIRECTORY",
        "GIT_PREFIX",
        "GIT_QUARANTINE_PATH",
        "GIT_WORK_TREE",
    ] {
        command.env_remove(name);
    }
    command
}
