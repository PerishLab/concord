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

pub struct Checkout<'a> {
    root: &'a Path,
}

pub struct Seat {
    pub identity: PathBuf,
    pub branch: String,
}

pub fn at(root: &Path) -> Checkout<'_> {
    Checkout { root }
}

impl Checkout<'_> {
    #[locus::trace(with = crate::observation::view())]
    pub fn text(&self, args: &[&str]) -> Result<String> {
        let root = git_path(self.root);
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
    pub fn seat(&self) -> Result<Seat> {
        let text = self.text(&[
            "rev-parse",
            "--path-format=absolute",
            "--git-common-dir",
            "--abbrev-ref",
            "HEAD",
        ])?;
        let mut lines = text.lines();
        let identity = lines
            .next()
            .ok_or_else(|| Error::new("git seat is missing common directory"))?;
        let branch = lines
            .next()
            .ok_or_else(|| Error::new("git seat is missing branch"))?;
        if lines.next().is_some() {
            return Err(Error::new("git seat produced unexpected output"));
        }
        Ok(Seat {
            identity: PathBuf::from(identity).canonicalize()?,
            branch: branch.into(),
        })
    }

    #[locus::trace(with = crate::observation::view())]
    pub fn branch(&self) -> Result<String> {
        self.text(&["symbolic-ref", "--short", "HEAD"])
    }

    pub fn clean(&self) -> Result<bool> {
        Ok(self
            .text(&["status", "--porcelain=v1", "--untracked-files=all"])?
            .is_empty())
    }

    pub fn exists(&self, branch: &str) -> Result<bool> {
        let reference = format!("refs/heads/{branch}");
        let root = git_path(self.root);
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

    pub fn landing(&self, source: &Path) -> Result<Option<Landing>> {
        let member_head = self.text(&["rev-parse", "HEAD"])?;
        let source = at(source);
        let source_head = source.text(&["rev-parse", "HEAD"])?;
        let status = command()
            .arg("-C")
            .arg(git_path(source.root))
            .args(["merge-base", "--is-ancestor", &member_head, &source_head])
            .status()
            .map_err(|error| Error::new(format!("cannot run git: {error}")))?;
        if status.success() {
            return Ok(Some(Landing::Reachable {
                member_head,
                source_head,
            }));
        }
        let member_tree = self.text(&["rev-parse", "HEAD^{tree}"])?;
        let source_tree = source.text(&["rev-parse", "HEAD^{tree}"])?;
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
        let member = git_path(member);
        let member = member
            .to_str()
            .ok_or_else(|| Error::new("member path is not utf8"))?;
        self.run(&["worktree", "repair", member])
    }

    pub fn add(&self, path: &Path, branch: &str, orphan: bool) -> Result<()> {
        let source = git_path(self.root);
        let source = source
            .to_str()
            .ok_or_else(|| Error::new("source path is not utf8"))?;
        let target = git_path(path);
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
            if let Err(cleanup) = self.rollback_add(path, branch) {
                return Err(Error::new(format!(
                    "git worktree add failed: {failure}; failed to roll back branch {branch}: {cleanup}"
                )));
            }
            return Err(Error::new(format!("git worktree add failed: {failure}")));
        }
        Ok(())
    }

    pub fn remove(&self, path: &Path) -> Result<()> {
        let path = git_path(path);
        let path = path
            .to_str()
            .ok_or_else(|| Error::new("member path is not utf8"))?;
        self.run(&["worktree", "remove", path])
    }

    fn rollback_add(&self, path: &Path, branch: &str) -> Result<()> {
        if !path.exists() && self.exists(branch)? {
            self.run(&["branch", "-D", branch])?;
        }
        Ok(())
    }
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
