use super::super::{evaluate, world};
use super::server::{State, handle};
use crate::spawn::Server;
use concord_core::acceptance::{Judgment, Marker, Reference, Target, Verification, digest};
use serde_json::{Value, json};
use std::io::Write;
use std::process::{Command, Output, Stdio};
use std::sync::{Arc, Mutex};

pub(super) struct World {
    root: tempfile::TempDir,
    server: Server,
    state: Arc<Mutex<State>>,
    data: Arc<Mutex<evaluate::World>>,
}

impl World {
    pub fn new() -> Self {
        let root = tempfile::tempdir().unwrap();
        let data = Arc::new(Mutex::new(evaluate::World::new()));
        let state = Arc::new(Mutex::new(State::new()));
        let shared = Arc::clone(&state);
        let fixtures = Arc::clone(&data);
        let server = Server::start(move |request| handle(request, &shared, &fixtures));
        Self {
            root,
            server,
            state,
            data,
        }
    }

    pub fn release(&self) {
        let mut data = self.data.lock().unwrap();
        let declaration = Marker::Declaration {
            target: Target::Release,
            promise: "Source and complete stable".into(),
        }
        .render()
        .unwrap();
        let body = data.page["data"]["repository"]["issue"]["body"]
            .as_str()
            .unwrap()
            .to_owned();
        let release = concord_core::acceptance::Release {
            marker: "v1.0.0".into(),
            distribution:
                "https://releases.example.org/v1/releases/stable/v1.0.0/distribution.json".into(),
            inclusion: vec!["manual inclusion".into()],
        };
        let closure = Marker::Closure {
            target: Target::Release,
            declaration: Reference {
                node: "IC_decl".into(),
                digest: digest(&declaration),
            },
            judgment: Judgment::Satisfied,
            verification: Verification::Manual,
            review: digest(&body),
            evidence: vec!["reviewed source".into()],
            remaining: vec![],
            release: Some(release),
        }
        .render()
        .unwrap();
        let issue = &mut data.page["data"]["repository"]["issue"];
        issue["labels"] = world::connection(vec![json!({"name": "acceptance:release"})], None, 1);
        issue["comments"] = world::connection(
            vec![
                world::entry("IC_decl", &declaration, 1),
                world::entry("IC_close", &closure, 2),
            ],
            None,
            2,
        );
        data.fresh["data"]["declaration"]["body"] = declaration.into();
        data.fresh["data"]["closure"]["body"] = closure.into();
    }

    pub fn mode(&self, mode: &str) {
        self.state.lock().unwrap().mode = mode.into();
    }

    pub fn state(&self) -> Value {
        self.state.lock().unwrap().json()
    }

    pub fn command(&self, operation: &str) -> Command {
        let mut command = crate::spawn::concord(self.root.path());
        self.server.attach(&mut command);
        command.args([
            "--home",
            "/deliberately-unavailable-estate",
            "issue",
            "close",
            operation,
            "PerishLab/concord#83",
        ]);
        command
    }

    pub fn run(&self, operation: &str, input: Option<&Value>) -> Output {
        let mut command = self.command(operation);
        command.arg("--json");
        if operation == "apply" {
            command.arg("--apply");
        }
        let output = output(command, input);
        assert!(!self.root.path().join("workspace").exists());
        assert!(!self.root.path().join("workspace/issues").exists());
        output
    }

    pub fn prepare(&self) -> Value {
        world::decoded(&self.run("prepare", None))
    }
}

pub(super) fn output(mut command: Command, input: Option<&Value>) -> Output {
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command.spawn().unwrap();
    if let Some(input) = input {
        child
            .stdin
            .take()
            .unwrap()
            .write_all(input.to_string().as_bytes())
            .unwrap();
    } else {
        drop(child.stdin.take());
    }
    child.wait_with_output().unwrap()
}
