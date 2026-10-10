use crate::{Error, Result, git};
use std::future::Future;
use std::path::Path;

#[cfg(test)]
#[path = "tests.rs"]
mod tests;

pub(super) struct Cleanup<'a> {
    pub source: &'a Path,
    pub path: &'a Path,
    pub branch: &'a str,
    pub head: &'a str,
}

impl Cleanup<'_> {
    pub async fn settle(&self, change: impl Future<Output = Result<()>>) -> Result<()> {
        if let Err(error) = git::at(self.source).remove(self.path) {
            return Err(self.failure(error));
        }
        if let Err(error) = change.await {
            return Err(self.failure(error));
        }
        Ok(())
    }

    fn failure(&self, error: Error) -> Error {
        let restored = self.restore();
        let recovery = match restored {
            Ok(()) => "original Member restored; retry the same exact revision".to_string(),
            Err(error) => format!(
                "Member restoration refused: {error}; inspect concord cookbook concord.member.cleanup"
            ),
        };
        Error::detailed(
            "concord.member.cleanup",
            format!("{error}; {recovery}"),
            serde_json::json!({"path": self.path, "branch": self.branch, "head": self.head}),
        )
    }

    pub fn restore(&self) -> Result<()> {
        let source = git::at(self.source);
        let head = source.text(&[
            "rev-parse",
            "--verify",
            &format!("refs/heads/{}^{{commit}}", self.branch),
        ])?;
        if head != self.head {
            return Err(Error::typed(
                "concord.member.recovery",
                "retained Member branch changed",
            ));
        }
        if self.path.exists() && source.registered(self.path)? {
            return self.verify();
        }
        self.vacant()?;
        source.restore(self.path, self.branch)?;
        self.verify()
    }

    fn vacant(&self) -> Result<()> {
        match std::fs::symlink_metadata(self.path) {
            Ok(metadata) if metadata.is_dir() && !metadata.file_type().is_symlink() => {
                if self.path.read_dir()?.next().is_some() {
                    return Err(Error::typed(
                        "concord.member.recovery",
                        "Member path contains payload; preserve and inspect it",
                    ));
                }
            }
            Ok(_) => {
                return Err(Error::typed(
                    "concord.member.recovery",
                    "Member path is not a direct directory",
                ));
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => (),
            Err(error) => return Err(error.into()),
        }
        Ok(())
    }

    fn verify(&self) -> Result<()> {
        let source = git::at(self.source);
        let member = git::at(self.path);
        if source.identity()? != member.identity()?
            || !source.registered(self.path)?
            || member.branch()? != self.branch
            || member.head()? != self.head
            || !member.clean()?
        {
            return Err(Error::typed(
                "concord.member.recovery",
                "Member payload or registration disagrees",
            ));
        }
        Ok(())
    }
}
