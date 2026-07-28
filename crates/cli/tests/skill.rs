use flate2::Compression;
use flate2::write::GzEncoder;
use std::fs;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn archive() -> Vec<u8> {
    let mut builder = tar::Builder::new(Vec::new());
    append(
        &mut builder,
        "concord/SKILL.md",
        b"---\nname: concord\ndescription: fixture\n---\n# Concord\n",
    );
    append(
        &mut builder,
        "concord/references/protocol.md",
        b"# Protocol\n",
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

fn serve(archive: Vec<u8>) -> String {
    let digest = plumb::skill::stamp(&archive);
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let port = listener.local_addr().expect("address").port();
    std::thread::spawn(move || {
        for stream in listener.incoming().take(2) {
            let mut stream = stream.expect("stream");
            let mut request = [0u8; 2048];
            let read = stream.read(&mut request).expect("request");
            let request = String::from_utf8_lossy(&request[..read]);
            let body = if request.contains("metadata.json") {
                format!(
                    r#"{{"releaseVersion":"0.3.0","artifacts":{{"skillTarGz":{{"name":"concord-skill.tar.gz","url":"http://127.0.0.1:{port}/concord-skill.tar.gz","sha256":"{digest}"}}}}}}"#
                )
                .into_bytes()
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

fn run(config: &Path, arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_concord"))
        .arg("--config")
        .arg(config)
        .args(arguments)
        .output()
        .expect("run concord")
}

fn config(root: &Path, releases: &str) -> PathBuf {
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

#[test]
fn product_skill_install_refusal_and_uninstall_close_the_loop() {
    let fixture = tempfile::tempdir().expect("fixture");
    let releases = serve(archive());
    let config_path = config(fixture.path(), &releases);
    let argument_home = fixture.path().join("argument-home");
    let argument_home_text = argument_home.display().to_string();
    let skill = fixture.path().join("agent/skills/concord");
    let skill_text = skill.display().to_string();

    let installed = run(
        &config_path,
        &[
            "--json",
            "--home",
            &argument_home_text,
            "skill",
            "install",
            "--version",
            "0.3.0",
            "--path",
            &skill_text,
        ],
    );
    assert!(
        installed.status.success(),
        "{}",
        String::from_utf8_lossy(&installed.stderr)
    );
    assert!(skill.join("SKILL.md").is_file());
    assert!(skill.join("references/protocol.md").is_file());
    let marker = fs::read_to_string(skill.join("metadata.json")).expect("marker");
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&marker).expect("marker json")["keeper"],
        "concord"
    );
    assert!(argument_home.join("state/skills.json").is_file(), "ledger");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let mode = fs::metadata(argument_home.join("state/skills.json"))
            .expect("ledger")
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(mode, 0o600, "ledger stays private");
    }
    assert!(
        !fixture.path().join("home/state/skills.json").exists(),
        "argument layer must override file home"
    );

    let listed = run(
        &config_path,
        &["--home", &argument_home_text, "skill", "list"],
    );
    assert!(listed.status.success());
    assert!(String::from_utf8_lossy(&listed.stdout).contains(&skill_text));

    let upgraded_releases = serve(archive());
    let upgraded_config = config(fixture.path(), &upgraded_releases);
    let upgraded = run(
        &upgraded_config,
        &[
            "--home",
            &argument_home_text,
            "skill",
            "upgrade",
            "--version",
            "0.3.0",
        ],
    );
    assert!(
        upgraded.status.success(),
        "{}",
        String::from_utf8_lossy(&upgraded.stderr)
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let mode = fs::metadata(argument_home.join("state/skills.json"))
            .expect("ledger")
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(mode, 0o600, "ledger replacement stays private");
    }

    fs::write(skill.join("metadata.json"), "{}").expect("spoil marker");
    let refused = run(
        &upgraded_config,
        &["--home", &argument_home_text, "skill", "uninstall"],
    );
    assert!(!refused.status.success(), "one-sided ownership must refuse");
    assert!(skill.is_dir(), "refused target survives");

    fs::write(skill.join("metadata.json"), marker).expect("restore marker");
    let removed = run(
        &upgraded_config,
        &["--home", &argument_home_text, "skill", "uninstall"],
    );
    assert!(
        removed.status.success(),
        "{}",
        String::from_utf8_lossy(&removed.stderr)
    );
    assert!(!skill.exists(), "managed target removed exactly");
}
