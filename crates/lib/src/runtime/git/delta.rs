use super::{Checkout, command, native};
use crate::{Error, Result};

impl Checkout<'_> {
    pub fn commit_exists(&self, revision: &str) -> Result<bool> {
        let root = native(self.root);
        let object = format!("{revision}^{{commit}}");
        let output = command()
            .arg("-C")
            .arg(&root)
            .args(["cat-file", "-e", &object])
            .output()
            .map_err(|error| Error::new(format!("cannot run git: {error}")))?;
        match output.status.code() {
            Some(0) => Ok(true),
            Some(_) => Ok(false),
            None => Err(Error::new("cannot inspect Git commit")),
        }
    }

    pub fn changed_paths(&self, base: &str, head: &str) -> Result<Vec<String>> {
        let root = native(self.root);
        let output = command()
            .arg("-C")
            .arg(&root)
            .args([
                "diff",
                "--name-only",
                "--no-renames",
                "-z",
                base,
                head,
                "--",
            ])
            .output()
            .map_err(|error| Error::new(format!("cannot run git: {error}")))?;
        if !output.status.success() {
            let error = String::from_utf8_lossy(&output.stderr).trim().to_string();
            return Err(Error::new(format!("git diff failed: {error}")));
        }
        let mut paths = output
            .stdout
            .split(|byte| *byte == 0)
            .filter(|path| !path.is_empty())
            .map(|path| {
                std::str::from_utf8(path)
                    .map(str::to_string)
                    .map_err(|error| Error::new(format!("Git path is not utf8: {error}")))
            })
            .collect::<Result<Vec<_>>>()?;
        paths.sort();
        paths.dedup();
        Ok(paths)
    }
}
