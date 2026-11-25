#![allow(dead_code)]

use serde_json::Value;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::Path;
use std::process::Command;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

pub(super) fn concord(scratch: &Path) -> Command {
    image(scratch, Path::new(env!("CARGO_BIN_EXE_concord")))
}

pub(super) fn image(scratch: &Path, binary: &Path) -> Command {
    let mut command = Command::new(binary);
    for (name, _) in std::env::vars() {
        if name.starts_with("CONCORD_") {
            command.env_remove(name);
        }
    }
    let home = scratch.join("home");
    command
        .env("HOME", &home)
        .env("USERPROFILE", &home)
        .env("LOCALAPPDATA", home.join("data"))
        .env_remove("GH_TOKEN")
        .env_remove("GITHUB_TOKEN")
        .env_remove("GITHUB_TOKEN_FILE")
        .env_remove("LOCUS_API")
        .env_remove("LOCUS_PARENT")
        .env_remove("CLAUDE_CODE_SESSION_ID")
        .env_remove("GROK_SESSION_ID")
        .env_remove("GROK_BOT_IDENTITY")
        .env_remove("CODEX_THREAD_ID")
        .env_remove("SANTI_SOUL_ID")
        .env_remove("SANTI_STRAND_ID")
        .env_remove("ANTIGRAVITY_CONVERSATION_ID");
    command
}

pub(super) struct Request {
    pub method: String,
    pub path: String,
    pub body: Vec<u8>,
}

impl Request {
    pub(super) fn json(&self) -> Value {
        serde_json::from_slice(&self.body).unwrap_or(Value::Null)
    }

    pub(super) fn query(&self) -> String {
        self.json()["query"].as_str().unwrap_or("").to_owned()
    }

    pub(super) fn variables(&self) -> Value {
        self.json()["variables"].clone()
    }
}

pub(super) struct Reply {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl Reply {
    pub(super) fn json(value: Value) -> Self {
        Self::status(200, value)
    }

    pub(super) fn status(status: u16, value: Value) -> Self {
        Self::raw(status, value.to_string().into_bytes())
    }

    pub(super) fn raw(status: u16, body: impl Into<Vec<u8>>) -> Self {
        Self {
            status,
            headers: Vec::new(),
            body: body.into(),
        }
    }

    pub(super) fn header(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.push((name.into(), value.into()));
        self
    }
}

pub(super) struct Server {
    address: SocketAddr,
    stop: Arc<AtomicBool>,
}

impl Drop for Server {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        let _ = TcpStream::connect(self.address);
    }
}

impl Server {
    pub(super) fn start(handler: impl Fn(Request) -> Reply + Send + Sync + 'static) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind loopback");
        let address = listener.local_addr().expect("loopback address");
        let stop = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&stop);
        let handler = Arc::new(handler);
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                if flag.load(Ordering::SeqCst) {
                    break;
                }
                let handler = Arc::clone(&handler);
                std::thread::spawn(move || serve(stream, &*handler));
            }
        });
        Self { address, stop }
    }

    pub(super) fn address(&self) -> String {
        self.address.to_string()
    }

    pub(super) fn attach<'a>(&self, command: &'a mut Command) -> &'a mut Command {
        command
            .env("CONCORD_GITHUB_LOOPBACK", self.address())
            .env("GITHUB_TOKEN", "fixture_token")
    }
}

fn serve(mut stream: TcpStream, handler: &(dyn Fn(Request) -> Reply + Send + Sync)) {
    let Some(request) = read(&mut stream) else {
        return;
    };
    let reply = handler(request);
    let mut head = format!(
        "HTTP/1.1 {} OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n",
        reply.status,
        reply.body.len()
    );
    for (name, value) in reply.headers {
        head.push_str(&format!("{name}: {value}\r\n"));
    }
    head.push_str("\r\n");
    let _ = stream.write_all(head.as_bytes());
    let _ = stream.write_all(&reply.body);
}

fn read(stream: &mut TcpStream) -> Option<Request> {
    let mut head = Vec::new();
    let mut byte = [0u8; 1];
    while !head.ends_with(b"\r\n\r\n") {
        if stream.read(&mut byte).ok()? == 0 {
            return None;
        }
        head.push(byte[0]);
    }
    let text = String::from_utf8_lossy(&head);
    let mut parts = text.lines().next()?.split_whitespace();
    let method = parts.next()?.to_owned();
    let path = parts.next()?.to_owned();
    let length = text
        .lines()
        .find_map(|line| {
            let (name, value) = line.split_once(':')?;
            name.eq_ignore_ascii_case("content-length")
                .then(|| value.trim().parse::<usize>().ok())
                .flatten()
        })
        .unwrap_or(0);
    let mut body = vec![0u8; length];
    stream.read_exact(&mut body).ok()?;
    Some(Request { method, path, body })
}
