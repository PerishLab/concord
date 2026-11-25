#[path = "calls/graph.rs"]
mod graph;
#[path = "calls/wire.rs"]
pub(super) mod wire;

use super::super::spawn::{self, Server};
use graph::Config;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output, Stdio};
use std::sync::{Arc, Mutex};

pub fn serve(root: &Path, coordinate: &str, stable: &str) -> Server {
    let config = Config {
        root: root.to_path_buf(),
        coordinate: coordinate.to_string(),
        stable: stable.to_string(),
        kind: "Task".to_string(),
        body: "## Outcome\nPipe exact plan".to_string(),
    };
    let lock = Arc::new(Mutex::new(()));
    Server::start(move |request| {
        if request.method == "POST" && request.path == "/graphql" {
            config.wait(&request);
            let guard = lock.lock().expect("mock lock");
            let reply = config.graphql(&request);
            drop(guard);
            return reply;
        }
        config.slow(&request);
        let guard = lock.lock().expect("mock lock");
        let reply = config.rest(&request);
        drop(guard);
        reply
    })
}

pub fn consume(space: &Path, server: Option<&Server>, plan: &[u8]) -> Output {
    start(space, server, plan)
        .wait_with_output()
        .expect("complete land")
}

pub fn start(space: &Path, server: Option<&Server>, plan: &[u8]) -> Child {
    feed(land(space, server, "-"), plan)
}

pub fn feed(mut command: Command, plan: &[u8]) -> Child {
    let mut consumer = command
        .stdin(Stdio::piped())
        .spawn()
        .expect("consume stored plan");
    consumer
        .stdin
        .take()
        .expect("land stdin")
        .write_all(plan)
        .expect("write stored plan");
    consumer
}

pub fn land(space: &Path, server: Option<&Server>, plan: &str) -> Command {
    let mut command = spawn::concord(space);
    if let Some(server) = server {
        server.attach(&mut command);
    }
    command
        .args(["--home", space.to_str().expect("root"), "--json"])
        .args([
            "issue",
            "delivery",
            "land",
            "PerishLab/probe#1",
            "--plan",
            plan,
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    command
}

pub fn tool(root: &Path, name: &str, body: String) -> PathBuf {
    let path = root.join(name);
    plumb::testing::executable(&path, format!("#!/bin/sh\n{body}\n"));
    path
}
