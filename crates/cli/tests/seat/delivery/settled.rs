use super::super::execution::unix::{self as execution, Facts, git, repository};
use super::super::spawn;
use super::provider::{consume, start};
use super::provider::{observer, projection, tool};
use super::unix::{guard, prepare, refused, success};
use serde_json::Value;
use std::path::Path;
use std::process::{Child, Stdio};
use std::time::{Duration, Instant};

#[test]
fn settled() {
    let fixture = tempfile::tempdir().expect("fixture");
    let space = fixture.path();
    let provider = open(space);
    let source = space.join("source");
    let lock = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(space.join(".concord.lock"))
        .expect("estate lock");
    lock.lock().expect("writer lock");
    let stray = space.join(".issues/I_stray/worktree");
    let path = stray.to_str().expect("stray path");
    git(&source, &["worktree", "add", "--detach", path]);
    let revision = revision(space);
    let mut delivery = prepare(space, &provider, &revision, true)
        .spawn()
        .expect("prepare");
    let mut audit = inspect(space);
    std::thread::sleep(Duration::from_secs(1));
    assert!(delivery.try_wait().expect("prepare state").is_none());
    assert!(audit.try_wait().expect("audit state").is_none());
    git(&source, &["worktree", "remove", "--force", path]);
    lock.unlock().expect("writer unlock");
    let prepared = delivery.wait_with_output().expect("prepare output");
    assert!(
        prepared.status.success(),
        "{}",
        String::from_utf8_lossy(&prepared.stderr)
    );
    let handoff: Value = serde_json::from_slice(&prepared.stdout).expect("handoff JSON");
    assert!(handoff["plan"].is_object(), "{handoff}");
    let audited = audit.wait_with_output().expect("audit output");
    assert!(
        audited.status.success(),
        "{}",
        String::from_utf8_lossy(&audited.stderr)
    );
    let report: Value = serde_json::from_slice(&audited.stdout).expect("audit JSON");
    assert_eq!(report["agreement"]["faults"], serde_json::json!([]));
}

#[test]
fn waits() {
    let fixture = tempfile::tempdir().expect("fixture");
    let probe = Probe::open(fixture.path());
    probe.write("checks-pending", "3");
    let mut land = start(probe.space, &probe.provider, &probe.plan());
    let waiting = Instant::now();
    while probe.read("checks-pending") != "0" {
        assert!(
            waiting.elapsed() < Duration::from_secs(60),
            "land never waited"
        );
        assert!(
            land.try_wait().expect("land state").is_none(),
            "land ended early"
        );
        std::thread::sleep(Duration::from_millis(50));
    }
    let lock = probe.integration();
    lock.try_lock()
        .expect("land holds no Integration lock while checks are pending");
    lock.unlock().expect("unlock");
    let landed = land.wait_with_output().expect("land output");
    assert!(
        landed.status.success(),
        "{}",
        String::from_utf8_lossy(&landed.stderr)
    );
    assert_eq!(probe.read("pull-state"), "MERGED");
}

#[test]
fn failed() {
    let fixture = tempfile::tempdir().expect("fixture");
    let probe = Probe::open(fixture.path());
    probe.write(
        "checks-rollup",
        r#"{"statusCheckRollup":[{"__typename":"CheckRun","name":"Guard","status":"COMPLETED","conclusion":"FAILURE"}]}"#,
    );
    let landed = consume(probe.space, &probe.provider, &probe.plan());
    refused(&landed, "concord.delivery.provider");
    let stderr = String::from_utf8_lossy(&landed.stderr);
    assert!(stderr.contains("failed checks: Guard"), "{stderr}");
    assert_eq!(probe.read("pull-state"), "OPEN");
}

struct Probe<'a> {
    space: &'a Path,
    provider: String,
}

impl<'a> Probe<'a> {
    fn open(space: &'a Path) -> Self {
        Self {
            provider: open(space),
            space,
        }
    }

    fn plan(&self) -> Vec<u8> {
        let prepared = prepare(self.space, &self.provider, &revision(self.space), true)
            .output()
            .expect("prepare");
        assert!(
            prepared.status.success(),
            "{}",
            String::from_utf8_lossy(&prepared.stderr)
        );
        prepared.stdout
    }

    fn write(&self, name: &str, content: &str) {
        std::fs::write(self.space.join(name), content).expect("provider state");
    }

    fn read(&self, name: &str) -> String {
        std::fs::read_to_string(self.space.join(name)).unwrap_or_default()
    }

    fn integration(&self) -> std::fs::File {
        let path = std::fs::read_dir(self.space.join(".concord"))
            .expect("estate directory")
            .map(|entry| entry.expect("entry").path())
            .find(|path| {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.starts_with("integration-"))
            })
            .expect("Integration lock");
        std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(path)
            .expect("Integration lock file")
    }
}

fn inspect(space: &Path) -> Child {
    spawn::concord(space)
        .args(["--root", space.to_str().expect("root"), "--json", "audit"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("audit")
}

fn open(space: &Path) -> String {
    success(space, &["issue", "bootstrap"]);
    let provider = tool(space, "provider", projection(space));
    let command = provider.to_str().expect("provider path");
    success(
        space,
        &[
            "issue",
            "attach",
            "PerishLab/probe#1",
            "--github-command",
            command,
        ],
    );
    let source = repository(space, "PerishLab/probe");
    let observer = observer(space);
    success(
        space,
        &[
            "integration",
            "register",
            "PerishLab/probe",
            "--path",
            source.to_str().expect("source path"),
            "--github-command",
            observer.to_str().expect("repository provider"),
        ],
    );
    let facts = Facts {
        node: "I_delivery",
        stable: "R_probe",
        coordinate: "PerishLab/probe",
        number: 1,
    };
    let start = execution::provider(space, facts);
    success(
        space,
        &[
            "member",
            "start",
            "PerishLab/probe#1",
            "--claim",
            "topic.md",
            "--revision",
            "0",
            "--github-command",
            start.to_str().expect("start provider"),
        ],
    );
    let remote = space.join("remote.git");
    let rewrite = format!("url.{}.insteadOf", remote.display());
    git(&source, &["config", "--unset-all", &rewrite]);
    let ssh = tool(
        space,
        "ssh",
        format!(
            "for last; do :; done\nexec sh -c \"$(printf '%s' \"$last\" | sed \"s|'/PerishLab/probe'|'{}'|\")\"",
            remote.display()
        ),
    );
    let origin = "ssh://git@github.com/PerishLab/probe";
    git(&source, &["remote", "set-url", "origin", origin]);
    git(
        &source,
        &["config", "core.sshCommand", ssh.to_str().expect("ssh")],
    );
    git(&source, &["config", "ssh.variant", "simple"]);
    let member = space.join(".issues/I_delivery/worktree");
    std::fs::write(member.join("topic.md"), "settled\n").expect("member delta");
    git(&member, &["add", "topic.md"]);
    git(&member, &["commit", "-m", "delivery"]);
    guard(&member, "PerishLab/probe");
    let revision = revision(space);
    success(
        space,
        &[
            "member",
            "prove",
            "PerishLab/probe#1",
            "--revision",
            &revision,
        ],
    );
    provider.to_str().expect("provider path").to_string()
}

fn revision(space: &Path) -> String {
    let shown = success(space, &["issue", "show", "PerishLab/probe#1"]);
    shown["anchor"]["revision"].to_string()
}
