use flate2::Compression;
use flate2::write::GzEncoder;
use std::fs;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

pub(super) fn archive() -> Vec<u8> {
    let mut builder = tar::Builder::new(Vec::new());
    append(
        &mut builder,
        "concord/SKILL.md",
        b"---\nname: concord\ndescription: fixture\n---\n# Concord\n",
    );
    let tar = builder.into_inner().expect("tar");
    let mut zip = GzEncoder::new(Vec::new(), Compression::default());
    zip.write_all(&tar).expect("compress");
    zip.finish().expect("finish")
}

fn append(builder: &mut tar::Builder<Vec<u8>>, path: &str, body: &[u8]) {
    let mut header = tar::Header::new_gnu();
    header.set_size(body.len() as u64);
    header.set_mode(0o644);
    header.set_cksum();
    builder
        .append_data(&mut header, path, body)
        .expect("append");
}

pub(super) fn serve(archive: Vec<u8>) -> String {
    let digest = plumb::skill::stamp(&archive);
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let port = listener.local_addr().expect("address").port();
    std::thread::spawn(move || {
        for stream in listener.incoming().take(16) {
            let mut stream = stream.expect("stream");
            let mut request = [0u8; 2048];
            let read = stream.read(&mut request).expect("request");
            let request = String::from_utf8_lossy(&request[..read]);
            let stable = seal(port, &digest, "stable", "v0.3.0");
            let body = if request.contains("/v1/channels/stable.json") {
                format!(
                    r#"{{"schema":1,"channel":"stable","releaseVersion":"v0.3.0","seal":{{"name":"seal.json","url":"http://127.0.0.1:{port}/v1/releases/stable/v0.3.0/seal.json","sha256":"{}"}}}}"#,
                    plumb::skill::stamp(&stable)
                )
                .into_bytes()
            } else if let Some((channel, version)) = route(&request) {
                seal(port, &digest, channel, version)
            } else {
                archive.clone()
            };
            let head = format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            );
            stream.write_all(head.as_bytes()).expect("head");
            stream.write_all(&body).expect("body");
        }
    });
    format!("http://127.0.0.1:{port}")
}

fn seal(port: u16, digest: &str, channel: &str, version: &str) -> Vec<u8> {
    format!(
        r#"{{"schema":1,"channel":"{channel}","releaseVersion":"{version}","artifacts":{{"skill":{{"name":"concord-skill.tar.gz","url":"http://127.0.0.1:{port}/concord-skill.tar.gz","sha256":"{digest}"}}}}}}"#
    )
    .into_bytes()
}

fn route(request: &str) -> Option<(&str, &str)> {
    let rest = request.split("/v1/releases/").nth(1)?;
    let mut parts = rest.split('/');
    Some((parts.next()?, parts.next()?))
}

pub(super) fn run(config: &Path, arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_concord"))
        .arg("--config")
        .arg(config)
        .args(arguments)
        .output()
        .expect("run concord")
}

pub(super) fn config(root: &Path, releases: &str) -> PathBuf {
    let path = root.join("concord.toml");
    fs::write(
        &path,
        format!(
            "home = {:?}\nreleases = {:?}\n",
            root.join("home"),
            releases
        ),
    )
    .expect("config");
    path
}
