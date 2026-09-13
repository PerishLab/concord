use plumb::depot::v3::{Identity, Kind, Manifest, Marker, Object, Pointer, Publication};
use std::collections::BTreeMap;
use std::fs;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::Output;
use std::sync::{Arc, Mutex};

pub(super) struct Server {
    source: String,
    routes: Arc<Mutex<BTreeMap<String, Vec<u8>>>>,
}

impl std::ops::Deref for Server {
    type Target = str;

    fn deref(&self) -> &str {
        &self.source
    }
}

impl Server {
    pub(super) fn update(&self, body: Vec<u8>) {
        self.routes
            .lock()
            .unwrap()
            .extend(routes(&self.source, body));
    }
}

pub(super) fn version() -> String {
    format!("v{}", env!("CARGO_PKG_VERSION"))
}

pub(super) fn brief() -> Vec<u8> {
    b"---\nname: concord\ndescription: fixture\n---\n# Concord\n".to_vec()
}

pub(super) fn serve(body: Vec<u8>) -> Server {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let source = format!("http://{}", listener.local_addr().expect("address"));
    let held = Arc::new(Mutex::new(routes(&source, body)));
    let routes = Arc::clone(&held);
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let mut stream = stream.expect("stream");
            let mut request = [0u8; 4096];
            let read = stream.read(&mut request).expect("request");
            let request = String::from_utf8_lossy(&request[..read]);
            let path = request.split_whitespace().nth(1).expect("path");
            let routes = routes.lock().unwrap();
            let body = routes.get(path);
            let status = if body.is_some() {
                "200 OK"
            } else {
                "404 Not Found"
            };
            let body = body.map(Vec::as_slice).unwrap_or_default();
            let head = format!(
                "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            );
            stream.write_all(head.as_bytes()).expect("head");
            stream.write_all(body).expect("body");
        }
    });
    Server {
        source,
        routes: held,
    }
}

fn routes(source: &str, body: Vec<u8>) -> BTreeMap<String, Vec<u8>> {
    let manifest = Manifest::new(
        Identity {
            product: "concord".into(),
            channel: "stable".into(),
            version: version(),
            marker: Marker {
                name: version(),
                sha256: "a".repeat(64),
            },
            kind: Kind::Skill,
        },
        vec![Object {
            path: "SKILL.md".into(),
            sha256: plumb::skill::stamp(&body),
            size: body.len() as u64,
            media: "text/markdown".into(),
            executable: false,
        }],
    )
    .expect("manifest");
    let pointer = Pointer::new(
        &manifest,
        Publication {
            source,
            prior: None,
            created: "2026-09-13T00:00:00Z".into(),
        },
    )
    .expect("pointer");
    let path = pointer.manifest.url.strip_prefix(source).expect("source");
    let base = path.strip_suffix("/manifest.json").expect("manifest path");
    BTreeMap::from([
        (
            format!("/channels/stable/skills/versions/{}/latest.json", version()),
            pointer.encode().expect("pointer"),
        ),
        (path.to_string(), manifest.encode().expect("manifest")),
        (format!("{base}/objects/SKILL.md"), body),
    ])
}

pub(super) fn run(config: &Path, arguments: &[&str]) -> Output {
    crate::spawn::concord(config.parent().expect("scratch"))
        .arg("--config")
        .arg(config)
        .args(arguments)
        .output()
        .expect("run concord")
}

pub(super) fn config(root: &Path, depot: &str) -> PathBuf {
    let path = root.join("concord.toml");
    fs::write(
        &path,
        format!(
            "home = {:?}\ndepot = {:?}\nreleases = \"http://127.0.0.1:1\"\n",
            root.join("home"),
            depot
        ),
    )
    .expect("config");
    path
}
