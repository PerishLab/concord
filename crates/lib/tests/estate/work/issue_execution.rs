use concord_core::{
    Admission, Coordinate, IssueAttach, IssueDeclaration, IssueImport, IssueNarrowing,
    IssueProving, IssueRelease, IssueRetirement, Reconcile, Seat,
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
    let member = estate
        .attach_issue(&IssueAttach {
            issue: issue.clone(),
            name: "delivery".to_string(),
            source: source.clone(),
            branch: None,
            claims: vec!["crates".to_string()],
            revision: 0,
        })
        .await
        .expect("attach Issue Member")
        .member;
    assert_eq!(member.issue, issue);
    assert_eq!(member.node, "I_execution");
    assert_eq!(member.branch, format!("issue-{}-delivery", 1));
    let path = temp.path().join(".issues/I_execution/members/delivery");
    assert!(path.join(".git").is_file());

    std::fs::create_dir(path.join("crates")).expect("claimed directory");
    std::fs::write(path.join("crates/note.md"), "bounded\n").expect("member delta");
    git(&path, &["add", "crates/note.md"]);
    git(&path, &["commit", "-m", "member delta"]);
    let proved = estate
        .prove_issue(&IssueProving {
            issue: issue.clone(),
            member: "delivery".to_string(),
            revision: 1,
        })
        .await
        .expect("prove Issue Member");
    assert!(proved.proof.is_some());

    let narrowed = estate
        .narrow_issue(&IssueNarrowing {
            issue: issue.clone(),
            member: "delivery".to_string(),
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
            member: "delivery".to_string(),
            revision: 3,
        })
        .await
        .expect("re-prove narrowed Member");

    for (number, revision) in [(31, 4), (32, 5)] {
        estate
            .refer_issue(&IssueDeclaration {
                issue: issue.clone(),
                member: "delivery".to_string(),
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
        .issue_member_status(&issue, "delivery")
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

    let moved = coordinate("PerishLab/plumb#40");
    estate
        .reconcile(&Reconcile {
            anchor: issue,
            node: "I_execution".to_string(),
            coordinate: moved.clone(),
            revision: 6,
        })
        .await
        .expect("reconcile transferred Issue");
    assert!(path.is_dir());
    assert_eq!(
        estate
            .issue_member(&moved, "delivery")
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
            member: "delivery".to_string(),
            revision: 8,
        })
        .await
        .expect_err("unlanded Member must not release");
    assert_eq!(refusal.code(), "concord.member.unlanded");
    assert_eq!(
        estate
            .retire_issue(&IssueRetirement {
                issue: moved.clone(),
                member: "delivery".to_string(),
                artifacts: vec!["evidence".to_string()],
                revision: 8,
            })
            .await
            .expect("retire unlanded Issue Member"),
        9
    );
    assert_eq!(
        estate
            .issue_member(&moved, "delivery")
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
