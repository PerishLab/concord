use super::{evaluate::World, world};
use serde_json::{Value, json};
use std::path::Path;
use std::process::Output;

const HEAD: &str = "0123456789abcdef0123456789abcdef01234567";

fn page() -> Value {
    let pull = json!({"id": "PR_one", "number": 83,
        "url": "https://github.com/PerishLab/concord/pull/83", "head": HEAD,
        "body": "Refs PerishLab/concord#83", "state": "OPEN", "updated": "2026-10-07T00:00:00Z",
        "references": world::connection(vec![], None, 0),
    });
    json!({"data": {"repository": {"id": "R_concord", "pull": pull}}})
}

fn link() -> Value {
    let repository = json!({"name": "concord", "owner": {"login": "PerishLab"}});
    json!({"id": "I_issue", "number": 83, "url": "https://github.com/PerishLab/concord/issues/83", "repository": repository})
}

fn invoke(scratch: &Path, provider: &Path) -> Output {
    let mut command = world::command(scratch, provider, "closing");
    command.args(["--head", HEAD]);
    command.env_remove("CODEX_THREAD_ID");
    command.env_remove("CLAUDE_SESSION_ID");
    command.output().unwrap()
}

#[test]
fn refs() {
    let scratch = tempfile::tempdir().unwrap();
    let page = page();
    let provider = world::provider(
        scratch.path(),
        &[page.clone(), page.clone(), page.clone(), page],
    );
    let report = world::decoded(&invoke(scratch.path(), &provider));
    assert_eq!(report["closing"]["passed"], true);
    assert_eq!(report["closing"]["pull"]["head"], HEAD);
    assert_eq!(report["closing"]["issues"], json!([]));
    assert!(!scratch.path().join(".concord").exists());
    assert_eq!(report["closing"]["digest"].as_str().unwrap().len(), 64);
    let mut bound = report["closing"].clone();
    let digest = bound.as_object_mut().unwrap().remove("digest").unwrap();
    assert_eq!(
        digest,
        concord_core::acceptance::digest(&serde_json::to_string(&bound).unwrap())
    );
}

#[test]
fn head() {
    let scratch = tempfile::tempdir().unwrap();
    let mut page = page();
    page["data"]["repository"]["pull"]["head"] = "f".repeat(40).into();
    let provider = world::provider(scratch.path(), &[page]);
    world::refused(
        &invoke(scratch.path(), &provider),
        "concord.acceptance.head",
    );
}

#[test]
fn malformed() {
    for mode in ["provider", "identity", "truncated", "reply", "duplicate"] {
        let scratch = tempfile::tempdir().unwrap();
        let mut page = page();
        let errors = json!([{"message": "partial"}]);
        let pull = &mut page["data"]["repository"]["pull"];
        let code = match mode {
            "provider" => {
                page["errors"] = errors;
                "provider"
            }
            "identity" => {
                pull["url"] = "https://github.com/PerishLab/concord/pull/84".into();
                "identity"
            }
            "truncated" => {
                pull["references"] = world::connection(vec![], Some("next"), 1);
                "truncated"
            }
            "duplicate" => {
                pull["references"] = world::connection(vec![link(), link()], None, 2);
                "identity"
            }
            _ => {
                pull["references"] = world::connection(vec![], None, 1);
                "reply"
            }
        };
        let provider = world::provider(scratch.path(), &[page]);
        let mut command = world::command(scratch.path(), &provider, "closing");
        command.args(["--head", HEAD, "--max-pages", "1"]);
        world::refused(
            &command.output().unwrap(),
            &format!("concord.acceptance.{code}"),
        );
    }
}

#[test]
fn changed() {
    for field in ["body", "updated"] {
        let scratch = tempfile::tempdir().unwrap();
        let first = page();
        let mut second = first.clone();
        second["data"]["repository"]["pull"][field] = "changed".into();
        let provider = world::provider(scratch.path(), &[first, second]);
        world::refused(
            &invoke(scratch.path(), &provider),
            "concord.acceptance.changed",
        );
    }
}

struct Closing {
    pull: Value,
    issue: World,
}

impl Closing {
    fn new() -> Self {
        let mut pull = page();
        pull["data"]["repository"]["pull"]["body"] = "Closes PerishLab/concord#83".into();
        pull["data"]["repository"]["pull"]["references"] = world::connection(vec![link()], None, 1);
        Self {
            pull,
            issue: World::new(),
        }
    }

    fn observations(&self) -> Vec<Value> {
        vec![
            self.issue.page.clone(),
            self.issue.page.clone(),
            self.issue.relations.clone(),
            self.issue.relations.clone(),
            self.issue.fresh.clone(),
            self.issue.fresh.clone(),
            self.issue.page.clone(),
            self.issue.page.clone(),
            self.issue.relations.clone(),
        ]
    }

    fn replies(&self) -> Vec<Value> {
        let mut replies = vec![self.pull.clone(), self.pull.clone()];
        replies.extend(self.observations());
        replies.push(self.pull.clone());
        replies.extend(self.observations());
        replies.push(self.pull.clone());
        replies
    }
}

#[test]
fn satisfied() {
    let closing = Closing::new();
    let scratch = tempfile::tempdir().unwrap();
    let provider = world::provider(scratch.path(), &closing.replies());
    let report = world::decoded(&invoke(scratch.path(), &provider));
    assert_eq!(report["closing"]["passed"], true);
    assert_eq!(
        report["closing"]["issues"][0]["evaluation"]["verification"],
        "manual"
    );
    assert!(!scratch.path().join(".concord").exists());
}

#[test]
fn unknown() {
    let mut closing = Closing::new();
    closing.issue.fresh["data"]["repository"]["issue"]["edited"] = "2026-09-30T00:00:00Z".into();
    let scratch = tempfile::tempdir().unwrap();
    let provider = world::provider(scratch.path(), &closing.replies());
    let output = invoke(scratch.path(), &provider);
    world::refused(&output, "concord.acceptance.closing");
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["closing"]["passed"], false);
    assert_eq!(
        report["closing"]["issues"][0]["evaluation"]["verdict"],
        "unknown"
    );
}

#[test]
fn unmet() {
    let mut closing = Closing::new();
    closing.issue.relations["data"]["repository"]["issue"]["blockers"] =
        world::connection(vec![json!({"id": "I_blocker", "state": "OPEN"})], None, 1);
    let scratch = tempfile::tempdir().unwrap();
    let provider = world::provider(scratch.path(), &closing.replies());
    let output = invoke(scratch.path(), &provider);
    world::refused(&output, "concord.acceptance.closing");
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(
        report["closing"]["issues"][0]["evaluation"]["verdict"],
        "unmet"
    );
}

#[test]
fn invalidated() {
    let closing = Closing::new();
    let mut replies = closing.replies();
    let last = replies.last_mut().unwrap();
    last["data"]["repository"]["pull"]["body"] = "Refs PerishLab/concord#83".into();
    let scratch = tempfile::tempdir().unwrap();
    let provider = world::provider(scratch.path(), &replies);
    world::refused(
        &invoke(scratch.path(), &provider),
        "concord.acceptance.changed",
    );
}

#[test]
fn moved() {
    let closing = Closing::new();
    let mut replies = closing.replies();
    for index in [16, 17] {
        replies[index]["data"]["repository"]["issue"]["edited"] = "2026-09-30T00:00:00Z".into();
    }
    let scratch = tempfile::tempdir().unwrap();
    let provider = world::provider(scratch.path(), &replies);
    world::refused(
        &invoke(scratch.path(), &provider),
        "concord.acceptance.changed",
    );
}

#[test]
fn identity() {
    let mut closing = Closing::new();
    closing.pull["data"]["repository"]["pull"]["references"]["nodes"][0]["id"] = "I_foreign".into();
    let scratch = tempfile::tempdir().unwrap();
    let provider = world::provider(scratch.path(), &closing.replies());
    world::refused(
        &invoke(scratch.path(), &provider),
        "concord.acceptance.identity",
    );
}

#[test]
fn bounds() {
    let scratch = tempfile::tempdir().unwrap();
    let provider = world::provider(scratch.path(), &[]);
    let mut command = world::command(scratch.path(), &provider, "closing");
    command.args(["--head", HEAD, "--max-pages", "0"]);
    world::refused(&command.output().unwrap(), "concord.acceptance.bounds");
    assert!(!scratch.path().join("calls").exists());
}

#[test]
fn legacy() {
    let mut closing = Closing::new();
    let issue = &mut closing.issue.page["data"]["repository"]["issue"];
    issue["labels"] = world::connection(vec![], None, 0);
    issue["comments"] = world::connection(vec![], None, 0);
    let observations = vec![
        closing.issue.page.clone(),
        closing.issue.page.clone(),
        closing.issue.relations.clone(),
        closing.issue.relations.clone(),
        closing.issue.relations.clone(),
    ];
    let mut replies = vec![closing.pull.clone(), closing.pull.clone()];
    replies.extend(observations.clone());
    replies.push(closing.pull.clone());
    replies.extend(observations);
    replies.push(closing.pull);
    let scratch = tempfile::tempdir().unwrap();
    let provider = world::provider(scratch.path(), &replies);
    let report = world::decoded(&invoke(scratch.path(), &provider));
    assert_eq!(report["closing"]["passed"], true);
    assert!(report["closing"]["issues"][0]["evaluation"]["target"].is_null());
    assert!(report["closing"]["issues"][0]["evaluation"]["verification"].is_null());
}
