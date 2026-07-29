use crate::{Error, Result};
use std::path::{Path, PathBuf};
use std::process::Command;

pub enum Landing {
    Reachable {
        member_head: String,
        source_head: String,
    },
    TreeEquivalent {
        member_head: String,
        source_head: String,
        member_tree: String,
        source_tree: String,
    },
}

pub fn text(root: &Path, args: &[&str]) -> Result<String> {
    let root = git_path(root);
    let output = command()
        .arg("-C")
        .arg(&*root)
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

pub fn branch_exists(root: &Path, branch: &str) -> Result<bool> {
    let reference = format!("refs/heads/{branch}");
    let root = git_path(root);
    let status = command()
        .arg("-C")
        .arg(&*root)
        .args(["show-ref", "--verify", "--quiet", &reference])
        .status()
        .map_err(|error| Error::new(format!("cannot run git: {error}")))?;
    match status.code() {
        Some(0) => Ok(true),
        Some(1) => Ok(false),
        _ => Err(Error::new(format!("cannot inspect target branch {branch}"))),
    }
}

pub fn landing(member: &Path, source: &Path) -> Result<Option<Landing>> {
    let member_head = text(member, &["rev-parse", "HEAD"])?;
    let source_head = text(source, &["rev-parse", "HEAD"])?;
    let source_arg = git_path(source);
    let status = command()
        .arg("-C")
        .arg(&*source_arg)
        .args(["merge-base", "--is-ancestor", &member_head, &source_head])
        .status()
        .map_err(|error| Error::new(format!("cannot run git: {error}")))?;
    if status.success() {
        return Ok(Some(Landing::Reachable {
            member_head,
            source_head,
        }));
    }
    let member_tree = text(member, &["rev-parse", "HEAD^{tree}"])?;
    let source_tree = text(source, &["rev-parse", "HEAD^{tree}"])?;
    if member_tree != source_tree {
        return Ok(None);
    }
    Ok(Some(Landing::TreeEquivalent {
        member_head,
        source_head,
        member_tree,
        source_tree,
    }))
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
    let member = git_path(member);
    let member = member
        .to_str()
        .ok_or_else(|| Error::new("member path is not utf8"))?;
    run(source, &["worktree", "repair", member])
}

pub fn add(source: &Path, path: &Path, branch: &str, orphan: bool) -> Result<()> {
    let source_arg = git_path(source);
    let source_arg = source_arg
        .to_str()
        .ok_or_else(|| Error::new("source path is not utf8"))?;
    let path_arg = git_path(path);
    let path_arg = path_arg
        .to_str()
        .ok_or_else(|| Error::new("member path is not utf8"))?;
    let mut args = vec!["-C", source_arg, "worktree", "add"];
    if orphan {
        args.extend(["--orphan", "-b", branch, path_arg]);
    } else {
        args.extend(["-b", branch, path_arg]);
    }
    let output = command().args(args).output()?;
    if !output.status.success() {
        let failure = String::from_utf8_lossy(&output.stderr).trim().to_string();
        if let Err(cleanup) = rollback_add(source, path, branch) {
            return Err(Error::new(format!(
                "git worktree add failed: {failure}; failed to roll back branch {branch}: {cleanup}"
            )));
        }
        return Err(Error::new(format!("git worktree add failed: {failure}")));
    }
    Ok(())
}

pub fn remove(source: &Path, path: &Path) -> Result<()> {
    let path = git_path(path);
    let path = path
        .to_str()
        .ok_or_else(|| Error::new("member path is not utf8"))?;
    run(source, &["worktree", "remove", path])
}

fn rollback_add(source: &Path, path: &Path, branch: &str) -> Result<()> {
    if !path.exists() && branch_exists(source, branch)? {
        run(source, &["branch", "-D", branch])?;
    }
    Ok(())
}

#[cfg(windows)]
fn git_path(path: &Path) -> PathBuf {
    if let Some(text) = path.to_str() {
        if let Some(rest) = text.strip_prefix("\\\\?\\UNC\\") {
            return PathBuf::from(format!("\\\\{rest}"));
        }
        if let Some(rest) = text.strip_prefix("\\\\?\\") {
            return PathBuf::from(rest);
        }
    }
    path.to_path_buf()
}

#[cfg(not(windows))]
fn git_path(path: &Path) -> PathBuf {
    path.to_path_buf()
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
