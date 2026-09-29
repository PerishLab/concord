use concord_core::{
    Admission, Coordinate, IssueAttach, IssueDeclaration, IssueImport, IssueNarrowing,
    IssueProving, IssueRelease, IssueRetirement, Reconcile, Register, Rename, Repository, Seat,
};
use std::path::Path;
use std::process::Command;

fn coordinate(raw: &str) -> Coordinate {
    Coordinate::parse(raw).expect("Issue coordinate")
}

#[tokio::test(flavor = "current_thread")]
async fn lifecycle() {
    let temp = tempfile::tempdir().expect("temporary Space");
    let source = temp.path().join("source");
    std::fs::create_dir(&source).expect("source directory");
    git(&source, &["init", "-b", "main"]);
    git(&source, &["config", "user.name", "Concord Test"]);
    git(
        &source,
        &["config", "user.email", "concord@example.invalid"],
    );
    std::fs::write(source.join("README.md"), "fixture\n").expect("fixture file");
    git(&source, &["add", "README.md"]);
    git(&source, &["commit", "-m", "fixture"]);
    git(
        &source,
        &[
            "remote",
            "add",
            "origin",
            "https://github.com/PerishLab/concord.git",
        ],
    );
    git(&source, &["update-ref", "refs/remotes/origin/main", "HEAD"]);
    git(
        &source,
        &["branch", "--set-upstream-to=origin/main", "main"],
    );

    let estate = Seat::new(temp.path())
        .bootstrap()
        .await
        .expect("bootstrap estate");
    let issue = coordinate("PerishLab/concord#26");
    estate
        .admit(&Admission {
            node: "I_execution".to_string(),
            coordinate: issue.clone(),
        })
        .await
        .expect("attach Issue");
    let mismatch = estate
        .register(&Register {
            node: "R_plumb".to_string(),
            repository: Repository::parse("PerishLab/plumb").expect("repository"),
            path: source.clone(),
        })
        .await
        .expect_err("origin repository mismatch");
    assert_eq!(mismatch.code(), "concord.integration.repository");
    estate
        .register(&Register {
            node: "R_concord".to_string(),
            repository: Repository::parse("PerishLab/concord").expect("repository"),
            path: source.clone(),
        })
        .await
        .expect("register Integration");
    let member = estate
        .attach_issue(&IssueAttach {
            issue: issue.clone(),
            claims: vec!["crates".to_string()],
            revision: 0,
        })
        .await
        .expect("attach Issue Member")
        .member;
    assert_eq!(member.issue, issue);
    assert_eq!(member.node, "I_execution");
    assert!(member.branch.starts_with("concord/issue-"));
    let path = temp.path().join(".issues/I_execution/worktree");
    assert!(path.join(".git").is_file());
    let duplicate = estate
        .attach_issue(&IssueAttach {
            issue: issue.clone(),
            claims: vec!["docs".to_string()],
            revision: 1,
        })
        .await
        .expect_err("one Anchor owns at most one Member");
    assert_eq!(duplicate.code(), "concord.member.reserved");

    std::fs::create_dir(path.join("crates")).expect("claimed directory");
    std::fs::write(path.join("crates/note.md"), "bounded\n").expect("member delta");
    git(&path, &["add", "crates/note.md"]);
    git(&path, &["commit", "-m", "member delta"]);
    let proved = estate
        .prove_issue(&IssueProving {
            issue: issue.clone(),
            revision: 1,
        })
        .await
        .expect("prove Issue Member");
    assert!(proved.proof.is_some());

    let narrowed = estate
        .narrow_issue(&IssueNarrowing {
            issue: issue.clone(),
            claims: vec!["crates/note.md".to_string()],
            revision: 2,
        })
        .await
        .expect("narrow Issue Member");
    assert_eq!(narrowed.member.claims, vec!["crates/note.md"]);
    assert!(narrowed.member.proof.is_none());
    estate
        .prove_issue(&IssueProving {
            issue: issue.clone(),
            revision: 3,
        })
        .await
        .expect("re-prove narrowed Member");

    for (number, revision) in [(31, 4), (32, 5)] {
        estate
            .refer_issue(&IssueDeclaration {
                issue: issue.clone(),
                provider: "github".to_string(),
                owner: "PerishLab".to_string(),
                repository: "concord".to_string(),
                number,
                revision,
            })
            .await
            .expect("declare pull coordinate");
    }
    let status = estate
        .issue_member_status(&issue)
        .await
        .expect("Issue Member status");
    assert_eq!(
        status
            .references
            .iter()
            .map(|reference| reference.number)
            .collect::<Vec<_>>(),
        vec![31, 32]
    );

    let transfer = estate
        .reconcile(&Reconcile {
            anchor: issue.clone(),
            node: "I_execution".to_string(),
            repository: "R_plumb".to_string(),
            coordinate: coordinate("PerishLab/plumb#40"),
            revision: 6,
        })
        .await
        .expect_err("refuse repository transfer");
    assert_eq!(transfer.code(), "concord.issue.repository_transfer");
    let moved = coordinate("PerishLab/concord-renamed#26");
    estate
        .rename(&Rename {
            node: "R_concord".to_string(),
            repository: Repository::parse("PerishLab/concord-renamed").expect("repository"),
        })
        .await
        .expect("reconcile repository rename");
    assert!(path.is_dir());
    assert_eq!(
        estate
            .issue_member(&moved)
            .await
            .expect("Member follows stable node")
            .node,
        "I_execution"
    );

    let artifact = temp.path().join("evidence.txt");
    std::fs::write(&artifact, "private\n").expect("Artifact source");
    let imported = estate
        .import_issue(&IssueImport {
            issue: moved.clone(),
            name: "evidence".to_string(),
            source: artifact,
            revision: 7,
        })
        .await
        .expect("import Issue Artifact");
    assert_eq!(
        imported.path,
        temp.path().join(".issues/I_execution/artifacts/evidence")
    );
    assert!(imported.path.join("evidence.txt").is_file());
    assert_eq!(
        estate.issue(&moved).await.expect("Issue anchor").revision,
        8
    );
    let refusal = estate
        .release_issue(&IssueRelease {
            issue: moved.clone(),
            revision: 8,
        })
        .await
        .expect_err("unlanded Member must not release");
    assert_eq!(refusal.code(), "concord.member.unlanded");
    assert_eq!(
        estate
            .retire_issue(&IssueRetirement {
                issue: moved.clone(),
                artifacts: vec!["evidence".to_string()],
                revision: 8,
            })
            .await
            .expect("retire unlanded Issue Member"),
        9
    );
    assert_eq!(
        estate
            .issue_member(&moved)
            .await
            .expect_err("retired Member")
            .code(),
        "concord.member.absent"
    );
    assert_eq!(
        estate
            .issue_artifacts(&moved)
            .await
            .expect("Artifacts")
            .len(),
        1
    );
    let foreign = temp.path().join("foreign");
    git(
        &source,
        &[
            "worktree",
            "add",
            "-b",
            "foreign",
            foreign.to_str().expect("foreign"),
        ],
    );
    let audit = estate.inspect().await.expect("audit estate");
    assert_eq!(audit.faults[0].code, "integration.worktree.unknown");
    assert_eq!(audit.faults[0].subject, foreign.display().to_string());
    assert!(audit.faults[0].message.contains(" at head "));
    git(
        &source,
        &["worktree", "remove", foreign.to_str().expect("foreign")],
    );
    let audit = estate.inspect().await.expect("audit estate");
    assert!(audit.faults.is_empty(), "{:?}", audit.faults);
}

fn git(root: &Path, arguments: &[&str]) {
    let status = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(arguments)
        .status()
        .expect("run Git");
    assert!(status.success());
}
