use super::{Fixture, fixture, released, request, snapshot};
use concord_core::authority::Plumb;
use concord_core::{IssueDeclaration, Reference, ReferenceKind, issue_delivery};
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::{Command, Stdio};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

const HOLD: Duration = Duration::from_secs(1);

#[tokio::test(flavor = "current_thread")]
async fn completion() {
    let compiled = released();
    let Fixture {
        estate,
        issue,
        _temp,
    } = fixture(compiled.producer(), compiled.depot()).await;
    let snapshot = snapshot();
    let plan = issue_delivery::prepare::<Plumb>(&estate, &request(&issue))
        .await
        .expect("prepare delivery");
    let source = _temp.path().join("source");
    let remote = _temp.path().join("remote.git");
    let ssh = _temp.path().join("ssh");
    std::fs::write(
        &ssh,
        format!(
            "#!/bin/sh\nfor last; do :; done\nexec sh -c \"$(printf '%s' \"$last\" | sed \"s|'/PerishLab/probe'|'{}'|\")\"\n",
            remote.display()
        ),
    )
    .expect("ssh tool");
    std::fs::set_permissions(&ssh, std::fs::Permissions::from_mode(0o700)).expect("ssh mode");
    git(
        &source,
        &[
            "remote",
            "set-url",
            "origin",
            "ssh://git@github.com/PerishLab/probe",
        ],
    );
    git(
        &source,
        &["config", "core.sshCommand", ssh.to_str().expect("ssh")],
    );
    git(&source, &["config", "ssh.variant", "simple"]);
    let tree = text(
        &source,
        &[
            "rev-parse",
            &format!("{}^{{tree}}", plan.delivery.candidate),
        ],
    );
    let message = text(
        &source,
        &["log", "-1", "--format=%B", &plan.delivery.candidate],
    );
    let merge = commit(&source, &tree, &plan.delivery.target, &message);
    git(
        &source,
        &["push", "origin", &format!("{merge}:refs/heads/main")],
    );
    let reference = Reference {
        kind: ReferenceKind::Change,
        provider: "github".into(),
        owner: "PerishLab".into(),
        repository: "probe".into(),
        number: 7,
    };
    estate
        .refer_issue(&IssueDeclaration {
            issue: issue.clone(),
            provider: reference.provider.clone(),
            owner: reference.owner.clone(),
            repository: reference.repository.clone(),
            number: reference.number,
            revision: 2,
        })
        .await
        .expect("record pull");
    git(&source, &["fetch", "origin", "main"]);
    git(&source, &["merge", "--ff-only", "origin/main"]);
    let settlement = issue_delivery::Settlement {
        authority: plan.authority.clone(),
        issue: issue.clone(),
        prepared: plan.revision,
        revision: 3,
        member: plan.member.clone(),
        boundary: plan.boundary.clone(),
        reference: reference.clone(),
        base: plan.delivery.base.clone(),
        candidate: plan.delivery.candidate.clone(),
        merge: merge.clone(),
        pushed: vec![
            plumb::delivery::Pushed {
                branch: plan.delivery.projection.clone(),
                head: plan.delivery.candidate.clone(),
            },
            plumb::delivery::Pushed {
                branch: plan.delivery.branch.clone(),
                head: plan.delivery.source.clone(),
            },
        ],
    };
    let integration = estate
        .integration(&issue)
        .await
        .expect("Integration")
        .guard(&estate)
        .expect("hold Integration");
    let wrong = estate
        .complete(
            &integration,
            &issue_delivery::Settlement {
                merge: plan.delivery.candidate.clone(),
                ..settlement.clone()
            },
        )
        .await
        .expect_err("different merge result");
    assert_eq!(wrong.code(), "concord.delivery.merge");
    let writer = hold(_temp.path(), &source);
    let began = Instant::now();
    let completed = estate
        .complete(&integration, &settlement)
        .await
        .expect("synchronize and release");
    assert!(began.elapsed() >= HOLD, "completion read a writer's state");
    writer.join().expect("writer");
    assert_eq!(completed.revision, 4);
    assert_eq!(completed.head, merge);
    assert!(completed.released);
    assert_eq!(
        completed.retired.deleted,
        [
            plan.delivery.projection.clone(),
            plan.delivery.branch.clone()
        ]
    );
    assert!(
        completed.retired.kept.is_empty(),
        "{:?}",
        completed.retired.kept
    );
    assert_eq!(
        text(
            &source,
            &["branch", "--list", plan.delivery.branch.as_str()]
        ),
        ""
    );
    assert_eq!(text(&source, &["rev-parse", "HEAD"]), completed.head);
    assert_eq!(
        estate
            .issue_member(&issue)
            .await
            .expect_err("Member released")
            .code(),
        "concord.member.absent"
    );
    let resumed = issue_delivery::resume::<Plumb>(&estate, &plan, &snapshot)
        .await
        .expect("derive released delivery");
    assert!(resumed.released);
    assert_eq!(resumed.revision, 4);
    let replay = estate
        .complete(
            &integration,
            &issue_delivery::Settlement {
                revision: 4,
                ..settlement
            },
        )
        .await
        .expect("released completion replay");
    assert_eq!(replay.revision, 4);
    assert!(!replay.released);
}

fn hold(space: &Path, source: &Path) -> JoinHandle<()> {
    let stray = space.join(".issues/I_stray/worktree");
    let path = stray.to_str().expect("stray path").to_string();
    git(source, &["worktree", "add", "--detach", &path]);
    let lock = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(space.join(".concord.lock"))
        .expect("estate lock");
    lock.lock().expect("writer lock");
    let source = source.to_path_buf();
    std::thread::spawn(move || {
        std::thread::sleep(HOLD);
        git(&source, &["worktree", "remove", "--force", &path]);
        lock.unlock().expect("writer unlock");
    })
}

fn git(root: &Path, args: &[&str]) {
    let status = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .status()
        .expect("git");
    assert!(status.success(), "git {args:?}");
}

fn text(root: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .expect("git");
    assert!(output.status.success());
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

fn commit(root: &Path, tree: &str, parent: &str, message: &str) -> String {
    let mut process = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["commit-tree", tree, "-p", parent, "-F", "-"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("git commit-tree");
    process
        .stdin
        .as_mut()
        .expect("commit-tree stdin")
        .write_all(message.as_bytes())
        .expect("commit message");
    let output = process.wait_with_output().expect("commit-tree output");
    assert!(output.status.success());
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}
