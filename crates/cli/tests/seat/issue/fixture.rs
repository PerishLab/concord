use super::super::execution::unix::{Facts, git, provider, repository};
use super::super::spawn;
use serde_json::{Value, json};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

pub(super) struct Seat<'a> {
    pub(super) space: &'a Path,
}

impl Seat<'_> {
    pub(super) fn setup(&self) -> PathBuf {
        let source = repository(self.space, "PerishLab/concord");
        std::fs::write(source.join("Cargo.lock"), "tracked fixture\n").expect("lock");
        git(&source, &["add", "Cargo.lock"]);
        git(&source, &["commit", "-m", "manifest fixture"]);
        git(&source, &["push", "origin", "main"]);
        let command = self.tool(
            "repository",
            json!({"node":"R_concord", "coordinate":"PerishLab/concord", "branch":"main"}),
        );
        self.success(&[
            "integration",
            "register",
            "PerishLab/concord",
            "--path",
            source.to_str().expect("source"),
            "--github-command",
            command.to_str().expect("provider"),
        ]);
        source
    }

    pub(super) fn attach(&self) {
        let command = self.tool("human",
            json!({"node":"I_human", "stable":"R_concord", "number":1, "url":"https://github.com/PerishLab/concord/issues/1", "state":"OPEN", "kind":"Feature", "updated_at":"now"}),
        );
        self.success(&[
            "issue",
            "attach",
            "PerishLab/concord#1",
            "--github-command",
            command.to_str().expect("provider"),
        ]);
    }

    pub(super) fn start(&self) -> PathBuf {
        provider(
            self.space,
            Facts {
                node: "I_human",
                stable: "R_concord",
                coordinate: "PerishLab/concord",
                number: 1,
            },
        )
    }

    pub(super) fn tool(&self, name: &str, reply: Value) -> PathBuf {
        let path = self.space.join(name);
        std::fs::write(&path, format!("#!/bin/sh\nprintf '%s\\n' '{reply}'\n")).expect("provider");
        let mut permissions = std::fs::metadata(&path).expect("provider").permissions();
        permissions.set_mode(0o700);
        std::fs::set_permissions(&path, permissions).expect("provider permissions");
        path
    }

    pub(super) fn run(&self, args: &[&str]) -> std::process::Output {
        spawn::concord(self.space)
            .args(["--root", self.space.to_str().expect("self.space"), "--json"])
            .args(args)
            .output()
            .expect("Concord")
    }

    pub(super) fn success(&self, args: &[&str]) -> Value {
        let output = self.run(args);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        serde_json::from_slice(&output.stdout).expect("Concord output")
    }
}
