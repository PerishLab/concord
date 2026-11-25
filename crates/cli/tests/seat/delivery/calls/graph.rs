use super::super::super::spawn::{Reply, Request};
use super::wire;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::time::Duration;

#[derive(Clone)]
pub(super) struct Config {
    pub root: PathBuf,
    pub coordinate: String,
    pub stable: String,
    pub kind: String,
    pub body: String,
}

impl Config {
    pub(super) fn kind(&self) -> String {
        read(&self.root, "issue-kind").unwrap_or_else(|| self.kind.clone())
    }

    pub(super) fn body(&self) -> String {
        read(&self.root, "issue-body").unwrap_or_else(|| self.body.clone())
    }

    pub(super) fn url(&self) -> String {
        format!("https://github.com/{}/issues/1", self.coordinate)
    }

    pub(super) fn wait(&self, request: &Request) {
        if request.query().contains("RunsealSnapshot") && self.root.join("provider-delay").exists()
        {
            std::thread::sleep(Duration::from_secs(2));
        }
    }

    pub(super) fn graphql(&self, request: &Request) -> Reply {
        let query = request.query();
        if query.contains("RunsealIssue") {
            return self.issue();
        }
        if query.contains("RunsealAdmission") {
            return self.admission();
        }
        if query.contains("RunsealSnapshot") {
            return self.snapshot();
        }
        if query.contains("RunsealTimeline") {
            return self.labels();
        }
        if query.contains("RunsealAcceptancePage") {
            return self.acceptance();
        }
        if query.contains("RunsealAcceptanceRelations") {
            return self.dependencies();
        }
        if query.contains("RunsealClassify") {
            return self.classify();
        }
        if query.contains("RunsealReadiness") {
            return self.readiness();
        }
        if query.contains("RunsealRollup") {
            return self.rollup();
        }
        Reply::status(404, json!({"message": "unknown graphql query"}))
    }

    fn issue(&self) -> Reply {
        let repo = json!({"nameWithOwner": self.coordinate, "issue": self.header()});
        data(repo)
    }

    fn admission(&self) -> Reply {
        let rules = json!([
            {"type": "DELETION"},
            {"type": "NON_FAST_FORWARD"},
            {"type": "PULL_REQUEST"},
        ]);
        let conditions = json!({"refName": {"include": ["~DEFAULT_BRANCH"], "exclude": []}});
        let ruleset = json!({
            "enforcement": "ACTIVE",
            "target": "BRANCH",
            "conditions": conditions,
            "bypassActors": {"totalCount": 0},
            "rules": {"nodes": rules, "pageInfo": {"hasNextPage": false}},
        });
        let issue = json!({
            "id": "I_delivery",
            "number": 1,
            "url": self.url(),
            "state": "OPEN",
            "issueType": {"name": "Feature"},
            "subIssuesSummary": {"total": 0},
            "labels": {"nodes": [], "pageInfo": {"hasNextPage": false}},
        });
        let repo = json!({
            "id": self.stable,
            "nameWithOwner": self.coordinate,
            "defaultBranchRef": {"name": "main"},
            "issue": issue,
            "rulesets": {"nodes": [ruleset], "pageInfo": {"hasNextPage": false}},
        });
        data(repo)
    }

    fn snapshot(&self) -> Reply {
        let status = self.root.join("status-call").exists();
        if status && self.root.join("projection-refusal").exists() {
            return Reply::status(502, json!({"message": "ghp_private"}));
        }
        if self.root.join("provider-failure").exists() {
            return Reply::status(500, json!({"message": "observe-failed"}));
        }
        let links: Vec<Value> = read(&self.root, "relationships")
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default();
        let blocking = json!({
            "totalCount": links.len(),
            "pageInfo": {"hasNextPage": false, "endCursor": null},
            "nodes": links,
        });
        let issue = json!({
            "id": "I_delivery",
            "number": 1,
            "url": self.url(),
            "title": "Stream delivery plan",
            "body": self.body(),
            "state": "OPEN",
            "updatedAt": "2026-09-29T00:00:00Z",
            "issueType": {"name": self.kind()},
            "repository": {"nameWithOwner": self.coordinate},
            "parent": null,
            "subIssues": empty(),
            "subIssuesSummary": {"total": 0, "completed": 0, "percentCompleted": 0},
            "blockedBy": empty(),
            "blocking": blocking,
            "timelineItems": empty(),
            "comments": empty(),
        });
        data(json!({"id": self.stable, "nameWithOwner": self.coordinate, "issue": issue}))
    }

    fn labels(&self) -> Reply {
        let (rows, more) = tags(&self.root);
        let total = rows.len() + usize::from(more);
        let cursor = more.then_some("cursor-next");
        let list = json!({"total": total, "rows": rows, "page": {"more": more, "cursor": cursor}});
        let issue = json!({"id": "I_delivery", "number": 1, "list": list});
        data(json!({"nameWithOwner": self.coordinate, "issue": issue}))
    }

    fn acceptance(&self) -> Reply {
        let (rows, _) = tags(&self.root);
        let comments = match read(&self.root, "declaration") {
            Some(body) if !body.is_empty() => vec![json!({
                "id": "IC_decl",
                "body": body,
                "url": format!("{}#issuecomment-1", self.url()),
            })],
            _ => Vec::new(),
        };
        let mut ticket = self.header();
        ticket["labels"] = page(rows);
        ticket["comments"] = page(comments);
        data(json!({"id": self.stable, "nameWithOwner": self.coordinate, "issue": ticket}))
    }

    fn dependencies(&self) -> Reply {
        let mut ticket = self.header();
        ticket["children"] = page(Vec::new());
        ticket["blockers"] = page(Vec::new());
        data(json!({"id": self.stable, "nameWithOwner": self.coordinate, "issue": ticket}))
    }

    fn classify(&self) -> Reply {
        match std::fs::read(self.root.join("branch-issues")) {
            Ok(bytes) => Reply::raw(200, bytes),
            Err(_) => Reply::status(500, json!({"message": "classify failed"})),
        }
    }

    fn readiness(&self) -> Reply {
        let pull = json!({
            "id": "PR_node",
            "number": 7,
            "head": self.candidate(),
            "state": "CLEAN",
        });
        data(json!({"id": self.stable, "nameWithOwner": self.coordinate, "pullRequest": pull}))
    }

    fn rollup(&self) -> Reply {
        wire::append(&self.root.join("calls"), "rollup\n");
        let head = self.candidate();
        let rollup = json!({"id": "SCR_node", "list": page(contexts(&self.root))});
        let commit = json!({"oid": head, "statusCheckRollup": rollup});
        let commits = json!({"nodes": [{"commit": commit}]});
        let pull = json!({"id": "PR_node", "number": 7, "head": head, "commits": commits});
        data(json!({"id": self.stable, "nameWithOwner": self.coordinate, "pullRequest": pull}))
    }

    fn header(&self) -> Value {
        json!({
            "id": "I_delivery",
            "number": 1,
            "title": "Stream delivery plan",
            "body": self.body(),
            "url": self.url(),
            "state": "OPEN",
            "reason": null,
            "updated": "2026-09-29T00:00:00Z",
            "writable": true,
            "kind": {"name": self.kind()},
            "repository": {"id": self.stable},
        })
    }
}

fn data(repository: Value) -> Reply {
    Reply::json(json!({"data": {"repository": repository}}))
}

fn contexts(root: &Path) -> Vec<Value> {
    if let Some(text) = read(root, "checks-rollup") {
        let value: Value = serde_json::from_str(&text).unwrap_or(Value::Null);
        let items = value["statusCheckRollup"]
            .as_array()
            .or_else(|| value.as_array())
            .cloned()
            .unwrap_or_default();
        return items
            .into_iter()
            .enumerate()
            .map(|(index, mut item)| {
                if item.get("id").is_none() {
                    item["id"] = json!(format!("C_{index}"));
                }
                item
            })
            .collect();
    }
    let left = read(root, "checks-pending")
        .and_then(|text| text.trim().parse::<u64>().ok())
        .unwrap_or(0);
    if left > 0 {
        let _ = std::fs::write(root.join("checks-pending"), (left - 1).to_string());
        return vec![json!({
            "__typename": "CheckRun", "id": "C_0", "name": "Guard",
            "status": "IN_PROGRESS", "conclusion": null, "detailsUrl": null,
        })];
    }
    vec![json!({
        "__typename": "CheckRun", "id": "C_0", "name": "Guard",
        "status": "COMPLETED", "conclusion": "SUCCESS", "detailsUrl": null,
    })]
}

fn tags(root: &Path) -> (Vec<Value>, bool) {
    let Some(text) = read(root, "labels") else {
        return (Vec::new(), false);
    };
    let value: Value = serde_json::from_str(&text).unwrap_or(Value::Null);
    let rows = value["nodes"].as_array().cloned().unwrap_or_default();
    let more = value["pageInfo"]["hasNextPage"].as_bool().unwrap_or(false);
    (rows, more)
}

fn page(rows: Vec<Value>) -> Value {
    json!({"total": rows.len(), "rows": rows, "page": {"more": false, "cursor": null}})
}

fn empty() -> Value {
    json!({"totalCount": 0, "pageInfo": {"hasNextPage": false, "endCursor": null}, "nodes": []})
}

fn read(root: &Path, name: &str) -> Option<String> {
    std::fs::read_to_string(root.join(name)).ok()
}
