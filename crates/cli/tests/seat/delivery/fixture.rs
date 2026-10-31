use super::super::execution::unix::{self as execution, Facts, git, repository};
use super::super::spawn;
use super::provider::{observer, projection, tool};
use super::unix::success;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

pub(super) struct Native<'a> {
    pub space: &'a Path,
    pub source: PathBuf,
    pub member: PathBuf,
    provider: PathBuf,
    pub issue: String,
    path: Option<std::ffi::OsString>,
}

impl<'a> Native<'a> {
    pub fn open(space: &'a Path, stable: &str) -> Self {
        Self::build(space, "PerishLab/wharf", stable)
    }

    pub fn governance(space: &'a Path, stable: &str) -> Self {
        Self::build(space, "PerishLab/.github", stable)
    }

    fn build(space: &'a Path, coordinate: &str, stable: &str) -> Self {
        let issue = format!("{coordinate}#1");
        success(space, &["issue", "bootstrap"]);
        let provider = tool(
            space,
            "provider",
            projection(space)
                .replace(r#"if [ "$1 $2" = "api -X" ]; then"#, &format!(r#"if [ "$1 $2" = "api -X" ]; then printf '%s\n' "$@" > '{}/status-call';"#, space.display()))
                .replace("  commit=$(", &format!("  if [ -f '{}/lose-proof' ]; then message='receipt missing'; fi\n  commit=$(", space.display()))
                .replace("PerishLab/probe", coordinate)
                .replace("R_probe", stable),
        );
        let command = provider.to_str().expect("provider");
        success(
            space,
            &["issue", "attach", &issue, "--github-command", command],
        );
        let source = repository(space, coordinate);
        if coordinate == "PerishLab/.github" {
            std::fs::write(
                source.join("AGENTS.md"),
                "This repository is maintained manually and is outside product governance.\n",
            )
            .expect("manual entry");
            std::fs::write(
                source.join("GOVERNANCE.md"),
                "This repository is maintained manually.\n",
            )
            .expect("manual policy");
        } else {
            checks(&source, space);
        }
        git(&source, &["add", "."]);
        git(&source, &["commit", "-m", "native checks"]);
        git(&source, &["push", "origin", "main"]);
        let observer = tool(
            space,
            "observer",
            std::fs::read_to_string(observer(space))
                .expect("observer")
                .replace("PerishLab/probe", coordinate)
                .replace("R_probe", stable),
        );
        success(
            space,
            &[
                "integration",
                "register",
                coordinate,
                "--path",
                source.to_str().expect("source"),
                "--github-command",
                observer.to_str().expect("observer"),
            ],
        );
        let start = execution::provider(
            space,
            Facts {
                node: "I_delivery",
                stable,
                coordinate,
                number: 1,
            },
        );
        success(
            space,
            &[
                "member",
                "start",
                &issue,
                "--claim",
                "topic.md",
                "--revision",
                "0",
                "--github-command",
                start.to_str().expect("start"),
            ],
        );
        let remote = space.join("remote.git");
        git(
            &source,
            &[
                "config",
                "--unset-all",
                &format!("url.{}.insteadOf", remote.display()),
            ],
        );
        let ssh = tool(
            space,
            "ssh",
            format!(
                "for last; do :; done\nexec sh -c \"$(printf '%s' \"$last\" | sed \"s|'/{coordinate}'|'{}'|\")\"",
                remote.display()
            ),
        );
        git(
            &source,
            &[
                "remote",
                "set-url",
                "origin",
                &format!("ssh://git@github.com/{coordinate}"),
            ],
        );
        git(
            &source,
            &["config", "core.sshCommand", ssh.to_str().expect("ssh")],
        );
        git(&source, &["config", "ssh.variant", "simple"]);
        let member = space.join(".issues/I_delivery/worktree");
        std::fs::write(member.join("topic.md"), "native\n").expect("change");
        git(&member, &["add", "topic.md"]);
        git(&member, &["commit", "-m", "native delivery"]);
        success(space, &["member", "prove", &issue, "--revision", "1"]);
        Self {
            space,
            source,
            member,
            provider,
            issue,
            path: None,
        }
    }

    pub fn tools(&mut self) {
        let directory = self.space.join("tools");
        std::fs::create_dir(&directory).expect("tools");
        let search = plumb::config::search(self.space).expect("tool search");
        for name in ["python3", "git"] {
            let actual = search
                .resolve(name)
                .expect("tool")
                .canonicalize()
                .expect("path");
            let quoted = actual.to_str().expect("tool path").replace('\'', "'\\''");
            plumb::testing::executable(
                &directory.join(name),
                format!("#!/bin/sh\nexec '{quoted}' \"$@\"\n"),
            );
        }
        let mut paths = vec![directory];
        paths.extend(std::env::split_paths(
            &std::env::var_os("PATH").expect("PATH"),
        ));
        self.path = Some(std::env::join_paths(paths).expect("tool PATH"));
    }

    pub fn prepare(&self, authority: &str) -> Output {
        let shown = success(self.space, &["issue", "show", &self.issue]);
        let revision = shown["anchor"]["revision"].to_string();
        self.command("prepare", authority)
            .args(["--revision", &revision])
            .output()
            .expect("prepare")
    }

    pub fn land(&self, authority: &str, plan: &[u8]) -> Output {
        let mut child = self
            .command("land", authority)
            .stdin(Stdio::piped())
            .spawn()
            .expect("land");
        child
            .stdin
            .take()
            .expect("stdin")
            .write_all(plan)
            .expect("plan");
        child.wait_with_output().expect("complete")
    }

    pub fn command(&self, verb: &str, authority: &str) -> Command {
        let mut command = spawn::concord(self.space);
        if let Some(path) = &self.path {
            command.env("PATH", path);
        }
        command
            .args([
                "--space",
                self.space.to_str().expect("space"),
                "--json",
                "issue",
                "delivery",
                verb,
                &self.issue,
                "--authority",
                authority,
                "--github-command",
                self.provider.to_str().expect("provider"),
            ])
            .env("GH_TOKEN", "native-test-sentinel")
            .env("CLOUDFLARE_API_TOKEN", "native-test-sentinel")
            .env("WHARF_TEST_SENTINEL", "native-test-sentinel")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        command
    }
}

fn checks(root: &Path, space: &Path) {
    std::fs::create_dir(root.join("scripts")).expect("scripts");
    std::fs::create_dir(root.join("tests")).expect("tests");
    std::fs::write(root.join("scripts/__init__.py"), "").expect("module");
    std::fs::write(root.join("tests/__init__.py"), "").expect("module");
    std::fs::write(root.join("scripts/selfcheck.py"), format!("import os\nfrom pathlib import Path\nassert os.environ['PATH']\nassert not any(k.startswith(('GH_', 'GITHUB_', 'CLOUDFLARE_', 'WHARF_', 'CONCORD_')) for k in os.environ)\nassert not Path({:?}).exists()\nopen({:?}, \"a\").write(\"gate\\n\")\n", space.join("fail-gate").to_str().expect("flag"), space.join("gate-runs").to_str().expect("runs"))).expect("selfcheck");
    std::fs::write(root.join("tests/test_native.py"), "import os\nimport unittest\nclass Native(unittest.TestCase):\n def test_world(self):\n  self.assertTrue(os.environ['PATH'])\n  self.assertFalse(any(k.startswith(('GH_', 'GITHUB_', 'CLOUDFLARE_', 'WHARF_', 'CONCORD_')) for k in os.environ))\n").expect("test");
}
