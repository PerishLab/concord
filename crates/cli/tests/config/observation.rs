use super::{expected, process};
use serde_json::Value;
use std::fs;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::PathBuf;
use std::process::Output;
use std::thread;

struct Observed(tempfile::TempDir);

impl Observed {
    fn new(config: &str) -> Self {
        let home = tempfile::tempdir().expect("home");
        let file = expected(home.path());
        fs::create_dir_all(file.parent().expect("config directory")).expect("config directory");
        fs::write(&file, config).expect("config");
        Self(home)
    }

    fn run(&self, pairs: &[(&str, &str)]) -> Output {
        let mut command = process(self.0.path(), &["config", "path"]);
        command.envs(pairs.iter().copied());
        command.output().expect("observe concord")
    }

    fn buffer(&self) -> PathBuf {
        expected(self.0.path())
            .parent()
            .expect("data home")
            .join("state")
            .join("locus")
    }
}

#[test]
fn retired() {
    let home = Observed::new("[locus]\nenabled = true\n");
    let output = home.run(&[("CONCORD_LOCUS_ENABLED", "true")]);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success());
    assert!(
        stderr.contains("CONCORD_LOCUS_ENABLED is retired"),
        "{stderr}"
    );
    assert!(!home.buffer().exists());
}

#[test]
fn disabled() {
    let home = Observed::new("");
    let output = home.run(&[]);
    assert!(output.status.success() && output.stderr.is_empty());
    assert!(!home.buffer().exists());
}

#[test]
fn configured() {
    let closed = TcpListener::bind("127.0.0.1:0").expect("listen");
    let endpoint = format!("http://{}", closed.local_addr().expect("address"));
    drop(closed);
    let home = Observed::new(&format!(
        "[locus]\nenabled = true\nendpoint = \"{endpoint}\"\n"
    ));
    let output = home.run(&[("CLAUDE_CODE_SESSION_ID", "session-1")]);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success());
    assert!(stderr.contains("reporter.registration"), "{stderr}");
    let active = fs::read_to_string(home.buffer().join("active.jsonl")).expect("buffer");
    let atoms: Vec<Value> = active
        .lines()
        .map(|line| serde_json::from_str(line).expect("atom"))
        .collect();
    assert!(atoms.len() >= 2);
    for atom in atoms {
        assert_eq!(atom["producer"], "concord");
        assert_eq!(atom["context"]["locus.trace"], "session-1");
    }
}

#[test]
fn unified() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("listen");
    let endpoint = format!("http://{}", listener.local_addr().expect("address"));
    thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept");
        let mut request = Vec::new();
        let mut chunk = [0; 4096];
        while !request.ends_with(b"}") {
            let read = stream.read(&mut chunk).expect("read");
            request.extend_from_slice(&chunk[..read]);
        }
        stream
            .write_all(b"HTTP/1.1 204 No Content\r\nconnection: close\r\n\r\n")
            .expect("reply");
    });
    let home = Observed::new("[locus]\nenabled = true\n");
    let output = home.run(&[("LOCUS_API", &endpoint)]);
    assert!(output.status.success() && output.stderr.is_empty());
    assert_eq!(
        fs::read_to_string(home.buffer().join("registration")).expect("marker"),
        endpoint
    );
}
