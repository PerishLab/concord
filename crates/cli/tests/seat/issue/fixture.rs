use super::super::execution::unix::{Facts, admission, git, record, repository, ticket};
use super::super::spawn::{self, Reply, Server};
use serde_json::Value;
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
        let server = Server::start(|_| Reply::json(record("R_concord", "PerishLab/concord")));
        self.success(
            Some(&server),
            &[
                "integration",
                "register",
                "PerishLab/concord",
                "--path",
                source.to_str().expect("source"),
            ],
        );
        source
    }

    pub(super) fn attach(&self) {
        let server = Server::start(|_| {
            Reply::json(ticket(
                &Facts {
                    node: "I_human",
                    stable: "R_concord",
                    coordinate: "PerishLab/concord",
                    number: 1,
                },
                "OPEN",
                "Feature",
                "now",
            ))
        });
        self.success(Some(&server), &["issue", "attach", "PerishLab/concord#1"]);
    }

    pub(super) fn start(&self, kind: &str) -> Server {
        let kind = kind.to_owned();
        Server::start(move |_| {
            Reply::json(admission(
                &Facts {
                    node: "I_human",
                    stable: "R_concord",
                    coordinate: "PerishLab/concord",
                    number: 1,
                },
                &kind,
            ))
        })
    }

    pub(super) fn run(&self, server: Option<&Server>, args: &[&str]) -> std::process::Output {
        let mut command = spawn::concord(self.space);
        command
            .args(["--home", self.space.to_str().expect("self.space"), "--json"])
            .args(args);
        if let Some(server) = server {
            server.attach(&mut command);
        }
        command.output().expect("Concord")
    }

    pub(super) fn success(&self, server: Option<&Server>, args: &[&str]) -> Value {
        let output = self.run(server, args);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        serde_json::from_slice(&output.stdout).expect("Concord output")
    }
}
