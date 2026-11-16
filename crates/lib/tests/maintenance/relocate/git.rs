use super::{Result, refuse};
use plumb::config::{Contract, Execution, Tool};
use std::collections::BTreeMap;
use std::path::Path;
use std::time::Duration;

pub(super) struct Git {
    context: Execution,
}

impl Git {
    pub(super) fn new(root: &Path) -> Result<Self> {
        let contract = Contract {
            inherit: ["PATH", "HOME", "USER", "LANG", "LC_ALL", "XDG_CONFIG_HOME"]
                .into_iter()
                .map(String::from)
                .collect(),
            managed: Vec::new(),
            reject: Vec::new(),
            bind: BTreeMap::new(),
        };
        let environment = plumb::config::environment(&contract).map_err(refuse)?;
        Ok(Self {
            context: Execution::new(environment, &["git".into()], root).map_err(refuse)?,
        })
    }

    pub(super) fn tool(&self) -> Result<Tool> {
        self.context
            .tools()
            .map_err(refuse)?
            .remove("git")
            .ok_or_else(|| refuse("missing Git binding"))
    }

    pub(super) async fn optional(&self, args: &[&str]) -> Result<Option<String>> {
        let mut command = self.context.command("git").map_err(refuse)?;
        radicula_git::detach(&mut command);
        command
            .args([
                "--no-optional-locks",
                "-c",
                "core.fsmonitor=false",
                "-c",
                "core.hooksPath=/dev/null",
            ])
            .args(args);
        let mut command = tokio::process::Command::from(command);
        command.kill_on_drop(true);
        let output = tokio::time::timeout(Duration::from_secs(10), command.output())
            .await
            .map_err(|_| refuse("Git operation timed out; preserve migration state"))??;
        if output.status.code() == Some(1) {
            return Ok(None);
        }
        if !output.status.success()
            || output.stdout.len() > 1048576
            || output.stderr.len() > 1048576
        {
            return Err(refuse(
                "Git operation refused or exceeded its output budget; preserve state",
            ));
        }
        String::from_utf8(output.stdout)
            .map(|text| Some(text.trim().into()))
            .map_err(|_| refuse("non-UTF8 Git result refuses"))
    }

    pub(super) async fn text(&self, args: &[&str]) -> Result<String> {
        self.optional(args)
            .await?
            .ok_or_else(|| refuse("required Git fact absent"))
    }

    pub(super) async fn repair(&self, target: &Path) -> Result<()> {
        let target = target
            .to_str()
            .ok_or_else(|| refuse("non-UTF8 worktree path refuses"))?;
        self.text(&[
            "-c",
            "worktree.useRelativePaths=false",
            "worktree",
            "repair",
            target,
        ])
        .await?;
        Ok(())
    }
}
