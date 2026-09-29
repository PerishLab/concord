use super::pull::{Pull, Report};
use super::settle::merge;
use plumb::guard::{Action, Descriptor};
use plumb::landing::{Guard, Preparation};
use sha2::{Digest as _, Sha256};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

const ORIGIN: &str = "ssh://git@github.com/PerishLab/probe";

struct Fixture {
    scratch: tempfile::TempDir,
    provider: PathBuf,
    preparation: Preparation,
    report: Report,
}

impl Fixture {
    fn new(tree: &str, parent: &str) -> Self {
        let scratch = tempfile::tempdir().expect("scratch");
        let remote = scratch.path().join("remote.git");
        let work = scratch.path().join("work");
        let ssh = tool(
            scratch.path().join("ssh"),
            &format!(
                "for last; do :; done\nexec sh -c \"$(printf '%s' \"$last\" | sed \"s|'/PerishLab/probe'|'{}'|\")\"",
                remote.display()
            ),
        );
        git(scratch.path(), &["init", "-q", "--bare", &show(&remote)]);
        git(scratch.path(), &["init", "-q", "-b", "main", &show(&work)]);
        for (key, value) in [
            ("user.name", "Concord Test"),
            ("user.email", "concord@example.invalid"),
            ("core.sshCommand", &show(&ssh)),
            ("ssh.variant", "simple"),
        ] {
            git(&work, &["config", key, value]);
        }
        git(&work, &["remote", "add", "origin", ORIGIN]);
        commit(&work, "README.md", "fixture");
        git(&work, &["push", "-q", "origin", "main"]);
        let target = text(&work, &["rev-parse", "HEAD"]);
        commit(&work, "topic.md", "Deliver topic");
        let guard = seal(&work);
        let candidate = text(&work, &["rev-parse", "HEAD"]);
        git(
            &work,
            &["push", "-q", "origin", "HEAD:refs/heads/land/topic"],
        );
        let provider = tool(
            scratch.path().join("gh"),
            &provider(&remote, &candidate, tree, parent),
        );
        Self {
            report: Report {
                pull: Pull {
                    id: "PR_node".into(),
                    number: 7,
                    url: "https://github.com/PerishLab/probe/pull/7".into(),
                    head: candidate.clone(),
                    title: "Deliver topic".into(),
                    body: "Refs PerishLab/probe#1".into(),
                },
                candidate: candidate.clone(),
                merged: false,
            },
            preparation: Preparation {
                root: work,
                base: "main".into(),
                target,
                branch: "topic".into(),
                projection: "land/topic".into(),
                source: candidate.clone(),
                candidate,
                title: "Deliver topic".into(),
                body: "Refs PerishLab/probe#1".into(),
                guard,
            },
            provider,
            scratch,
        }
    }

    async fn merge(&mut self) -> concord_core::Result<()> {
        merge(&mut self.report, &self.preparation, &self.provider, 30).await
    }

    fn remote(&self, reference: &str) -> String {
        text(
            &self.scratch.path().join("remote.git"),
            &["rev-parse", reference],
        )
    }
}

#[tokio::test]
async fn squash() {
    let mut fixture = Fixture::new("$candidate^{tree}", "$candidate^");
    fixture.merge().await.expect("squash lands");
    assert!(fixture.report.merged);
    let candidate = fixture.preparation.candidate.clone();
    assert_ne!(fixture.remote("main"), candidate);
    assert_eq!(fixture.remote("main^"), fixture.preparation.target);
    assert_eq!(
        fixture.remote("main^{tree}"),
        fixture.remote(&format!("{candidate}^{{tree}}"))
    );
}

#[tokio::test]
async fn tree() {
    refused(Fixture::new("$candidate^^{tree}", "$candidate^"), "tree").await;
}

#[tokio::test]
async fn moved() {
    refused(Fixture::new("$candidate^{tree}", "$candidate"), "moved").await;
}

async fn refused(mut fixture: Fixture, kind: &str) {
    let error = fixture.merge().await.expect_err("readback refuses");
    assert_eq!(error.code(), "concord.delivery.landed");
    let message = error.message();
    assert!(
        message.contains(&fixture.preparation.candidate),
        "{message}"
    );
    assert!(message.contains(&fixture.remote("main")), "{message}");
    assert!(message.contains(&format!("({kind})")), "{message}");
    assert!(!fixture.report.merged);
}

fn provider(remote: &Path, candidate: &str, tree: &str, parent: &str) -> String {
    format!(
        r#"[ "$1" = api ] && exit 0
[ "$1 $2" = "pr merge" ] || exit 1
number=$3
shift 3
while [ $# -gt 0 ]; do
  case "$1" in
    --squash) method=squash ;;
    -R) repository=$2; shift ;;
    --match-head-commit) head=$2; shift ;;
    --subject) subject=$2; shift ;;
    --body) body=$2; shift ;;
    *) echo "Merge commits are not allowed on this repository: $1" >&2; exit 1 ;;
  esac
  shift
done
candidate={candidate}
[ "$method $number $repository $head" = "squash 7 PerishLab/probe $candidate" ] || exit 1
export GIT_DIR='{}'
tree=$(git rev-parse "{tree}") && parent=$(git rev-parse "{parent}") || exit 1
commit=$(printf '%s\n\n%s\n' "$subject" "$body" | git -c user.name=GitHub -c user.email=noreply@github.com commit-tree "$tree" -p "$parent") || exit 1
git update-ref refs/heads/main "$commit""#,
        remote.display()
    )
}

fn seal(work: &Path) -> Guard {
    let tree = text(work, &["rev-parse", "HEAD^{tree}"]);
    let mut proof = Descriptor {
        schema: plumb::guard::SCHEMA.into(),
        repository: "PerishLab/probe".into(),
        tree: tree.clone(),
        plumb: "plumb-test".into(),
        depot: "depot-test".into(),
        platform: plumb::config::platform(),
        actions: vec![Action {
            name: "guard/test".into(),
            input: "3".repeat(64),
            world: "4".repeat(64),
        }],
        digest: String::new(),
    };
    let unsealed = serde_json::to_string(&proof).expect("claim");
    let claim = unsealed
        .strip_suffix(r#","digest":""}"#)
        .expect("digest is the last field");
    let digest = format!("{:x}", Sha256::digest(format!("{claim}}}")));
    proof.digest.clone_from(&digest);
    let token = proof.encode().expect("proof");
    let message = format!(
        "Deliver topic\n\nCarry the topic.\n\n{} {token}",
        plumb::guard::TRAILER
    );
    git(work, &["commit", "-q", "--amend", "-m", &message]);
    Guard {
        schema: plumb::guard::SCHEMA.into(),
        tree,
        digest,
    }
}

fn commit(work: &Path, file: &str, subject: &str) {
    std::fs::write(work.join(file), format!("{subject}\n")).expect("write");
    git(work, &["add", file]);
    git(work, &["commit", "-q", "-m", subject]);
}

fn tool(path: PathBuf, body: &str) -> PathBuf {
    std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).expect("tool");
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).expect("mode");
    path
}

fn show(path: &Path) -> String {
    path.to_str().expect("path").to_string()
}

fn git(root: &Path, arguments: &[&str]) {
    let status = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(arguments)
        .status()
        .expect("run Git");
    assert!(status.success(), "git {arguments:?}");
}

fn text(root: &Path, arguments: &[&str]) -> String {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(arguments)
        .output()
        .expect("run Git");
    assert!(output.status.success(), "git {arguments:?}");
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}
