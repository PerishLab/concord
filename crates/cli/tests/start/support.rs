use super::spawn::{self, Reply, Server};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

pub fn fact() -> Value {
    let page = json!({"hasNextPage": false});
    let labels = json!({"nodes": [], "pageInfo": page});
    let issue = json!({
        "id": "I_execution",
        "number": 26,
        "url": "https://github.com/PerishLab/concord/issues/26",
        "state": "OPEN",
        "issueType": {"name": "Feature"},
        "subIssuesSummary": {"total": 0},
        "labels": labels,
    });
    let names = json!({"include": ["~DEFAULT_BRANCH"], "exclude": []});
    let nodes = json!([
        {"type": "DELETION"},
        {"type": "NON_FAST_FORWARD"},
        {"type": "PULL_REQUEST"},
    ]);
    let rules = json!({"nodes": nodes, "pageInfo": page});
    let policy = json!({
        "enforcement": "ACTIVE",
        "target": "BRANCH",
        "conditions": {"refName": names},
        "bypassActors": {"totalCount": 0},
        "rules": rules,
    });
    let rulesets = json!({"nodes": [policy], "pageInfo": page});
    let repository = json!({
        "id": "R_concord",
        "nameWithOwner": "PerishLab/concord",
        "defaultBranchRef": {"name": "main"},
        "issue": issue,
        "rulesets": rulesets,
    });
    json!({"data": {"repository": repository}})
}

pub fn server(reply: Value) -> Server {
    Server::start(move |_| Reply::json(reply.clone()))
}

pub fn repository(root: &Path, coordinate: &str) -> PathBuf {
    let source = root.join("source");
    let remote = root.join("remote.git");
    std::fs::create_dir(&source).expect("source");
    std::fs::create_dir(&remote).expect("remote");
    git(&remote, &["init", "--bare"]);
    git(&source, &["init", "-b", "main"]);
    git(&source, &["config", "user.name", "Concord Test"]);
    git(
        &source,
        &["config", "user.email", "concord@example.invalid"],
    );
    std::fs::write(source.join("README.md"), "fixture\n").expect("fixture");
    git(&source, &["add", "README.md"]);
    git(&source, &["commit", "-m", "fixture"]);
    let github = format!("https://github.com/{coordinate}.git");
    git(&source, &["remote", "add", "origin", &github]);
    git(
        &source,
        &[
            "config",
            &format!("url.{}.insteadOf", remote.display()),
            &github,
        ],
    );
    git(&source, &["push", "-u", "origin", "main"]);
    source
}

pub fn git(root: &Path, arguments: &[&str]) {
    assert!(
        std::process::Command::new("git")
            .arg("-C")
            .arg(root)
            .args(arguments)
            .status()
            .expect("run Git")
            .success()
    );
}

pub fn success(space: &Path, arguments: &[&str], server: Option<&Server>) -> Value {
    let mut command = spawn::concord(space);
    command
        .args(["--home", space.to_str().expect("Space"), "--json"])
        .args(arguments);
    if let Some(server) = server {
        server.attach(&mut command);
    }
    let output = command.output().expect("run Concord");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("Concord JSON")
}
