use crate::{Error, Result};
use std::path::{Path, PathBuf};
use std::process::Command;

pub struct Checkout<'a> {
    root: &'a Path,
}

pub fn at(root: &Path) -> Checkout<'_> {
    Checkout { root }
}

impl Checkout<'_> {
    #[locus::trace(with = crate::observation::view())]
    pub fn text(&self, args: &[&str]) -> Result<String> {
        let root = native(self.root);
        let output = command()
            .arg("-C")
            .arg(&root)
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

    pub fn run(&self, args: &[&str]) -> Result<()> {
        self.text(args).map(|_| ())
    }

    #[locus::trace(with = crate::observation::view())]
    pub fn identity(&self) -> Result<PathBuf> {
        let path = self.text(&["rev-parse", "--path-format=absolute", "--git-common-dir"])?;
        PathBuf::from(path).canonicalize().map_err(Into::into)
    }

    #[locus::trace(with = crate::observation::view())]
    pub fn branch(&self) -> Result<String> {
        self.text(&["symbolic-ref", "--short", "HEAD"])
    }

    pub fn clean(&self) -> Result<bool> {
        let (tracked, untracked) = self.changes()?;
        Ok(tracked == 0 && untracked == 0)
    }

    pub fn changes(&self) -> Result<(usize, usize)> {
        let status = self.text(&["status", "--porcelain=v1", "--untracked-files=all"])?;
        let untracked = status.lines().filter(|line| line.starts_with("??")).count();
        Ok((status.lines().count() - untracked, untracked))
    }

    pub fn head(&self) -> Result<String> {
        self.text(&["rev-parse", "--verify", "HEAD^{commit}"])
    }

    pub fn merge(&self, left: &str, right: &str) -> Result<String> {
        self.text(&["merge-base", left, right])
    }

    pub fn ancestor(&self, left: &str, right: &str) -> Result<bool> {
        let root = native(self.root);
        let status = command()
            .arg("-C")
            .arg(&root)
            .args(["merge-base", "--is-ancestor", left, right])
            .status()
            .map_err(|error| Error::new(format!("cannot run git: {error}")))?;
        match status.code() {
            Some(0) => Ok(true),
            Some(1) => Ok(false),
            _ => Err(Error::new("cannot inspect Git ancestry")),
        }
    }

    pub fn tree(&self, revision: &str) -> Result<String> {
        self.text(&["rev-parse", &format!("{revision}^{{tree}}")])
    }

    pub fn upstream(&self, branch: &str) -> Result<Option<String>> {
        let reference = format!("refs/heads/{branch}");
        let value = self.text(&["for-each-ref", "--format=%(upstream:short)", &reference])?;
        Ok((!value.is_empty()).then_some(value))
    }

    pub fn divergence(&self, left: &str, right: &str) -> Result<(usize, usize)> {
        let range = format!("{left}...{right}");
        let value = self.text(&["rev-list", "--left-right", "--count", &range])?;
        let counts = value
            .split_whitespace()
            .map(str::parse::<usize>)
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|error| Error::new(format!("cannot read Git divergence: {error}")))?;
        match counts.as_slice() {
            [ahead, behind] => Ok((*ahead, *behind)),
            _ => Err(Error::new("Git divergence did not return two counts")),
        }
    }

    pub fn tracking(&self, revision: &str) -> Result<Vec<String>> {
        let mut refs = self
            .text(&[
                "for-each-ref",
                "--format=%(refname:short)",
                "--contains",
                revision,
                "refs/remotes",
            ])?
            .lines()
            .filter(|line| !line.is_empty())
            .map(str::to_string)
            .collect::<Vec<_>>();
        refs.sort();
        refs.dedup();
        Ok(refs)
    }

    pub fn exists(&self, branch: &str) -> Result<bool> {
        let reference = format!("refs/heads/{branch}");
        let root = native(self.root);
        let status = command()
            .arg("-C")
            .arg(&root)
            .args(["show-ref", "--verify", "--quiet", &reference])
            .status()
            .map_err(|error| Error::new(format!("cannot run git: {error}")))?;
        match status.code() {
            Some(0) => Ok(true),
            Some(1) => Ok(false),
            _ => Err(Error::new(format!("cannot inspect target branch {branch}"))),
        }
    }

    pub fn landed(&self, source: &Path) -> Result<bool> {
        let member = self.text(&["rev-parse", "HEAD"])?;
        let source = at(source);
        let origin = source.text(&["rev-parse", "HEAD"])?;
        let status = command()
            .arg("-C")
            .arg(native(source.root))
            .args(["merge-base", "--is-ancestor", &member, &origin])
            .status()
            .map_err(|error| Error::new(format!("cannot run git: {error}")))?;
        if status.success() {
            return Ok(true);
        }
        let member = self.text(&["rev-parse", "HEAD^{tree}"])?;
        let source = source.text(&["rev-parse", "HEAD^{tree}"])?;
        Ok(member == source)
    }

    #[locus::trace(with = crate::observation::view())]
    pub fn registered(&self, member: &Path) -> Result<bool> {
        let wanted = member.canonicalize()?;
        let list = self.text(&["worktree", "list", "--porcelain"])?;
        for line in list.lines() {
            if let Some(path) = line.strip_prefix("worktree ")
                && Path::new(path).canonicalize().ok().as_ref() == Some(&wanted)
            {
                return Ok(true);
            }
        }
        Ok(false)
    }

    pub fn repair(&self, member: &Path) -> Result<()> {
        let member = native(member);
        let member = member
            .to_str()
            .ok_or_else(|| Error::new("member path is not utf8"))?;
        self.run(&["worktree", "repair", member])
    }

    pub fn add(&self, path: &Path, branch: &str, orphan: bool) -> Result<()> {
        let source = native(self.root);
        let source = source
            .to_str()
            .ok_or_else(|| Error::new("source path is not utf8"))?;
        let target = native(path);
        let target = target
            .to_str()
            .ok_or_else(|| Error::new("member path is not utf8"))?;
        let mut args = vec!["-C", source, "worktree", "add"];
        if orphan {
            args.extend(["--orphan", "-b", branch, target]);
        } else {
            args.extend(["-b", branch, target]);
        }
        let output = command().args(args).output()?;
        if !output.status.success() {
            let failure = String::from_utf8_lossy(&output.stderr).trim().to_string();
            if let Err(cleanup) = self.rollback(path, branch) {
                return Err(Error::new(format!(
                    "git worktree add failed: {failure}; failed to roll back branch {branch}: {cleanup}"
                )));
            }
            return Err(Error::new(format!("git worktree add failed: {failure}")));
        }
        Ok(())
    }

    pub fn remove(&self, path: &Path) -> Result<()> {
        let path = native(path);
        let path = path
            .to_str()
            .ok_or_else(|| Error::new("member path is not utf8"))?;
        self.run(&["worktree", "remove", path])
    }

    fn rollback(&self, path: &Path, branch: &str) -> Result<()> {
        if !path.exists() && self.exists(branch)? {
            self.run(&["branch", "-D", branch])?;
        }
        Ok(())
    }
}

#[cfg(windows)]
fn native(path: &Path) -> PathBuf {
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
fn native(path: &Path) -> PathBuf {
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
