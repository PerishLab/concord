use super::super::super::spawn::{self, Reply, Request, Server};
use super::super::planning::overview;
pub(super) use super::data::today;
use super::data::{fields, issue, item, meta};
use serde_json::{Value, json};
use std::process::Output;
use std::sync::{Arc, Mutex};

#[test]
fn pages() {
    let mut selected = Vec::new();
    for (size, expected) in [(None, 5), (Some(20), 11), (Some(100), 3)] {
        let fixture = Fixture::new("ordinary", 208);
        let result = fixture.success("next", size, 20);
        let calls = fixture.calls("items");
        assert_eq!(calls.len(), expected, "page size {size:?}");
        assert!(calls.iter().all(|call| call["size"] == size.unwrap_or(50)));
        assert_eq!(
            fixture.calls("fields")[0]["size"],
            size.unwrap_or(50).min(20)
        );
        assert_eq!(fixture.calls("confirm").len(), 2);
        assert_eq!(fixture.calls("final").len(), 1);
        let next = &result["next"];
        assert_eq!(next["item"], "ITEM_139");
        assert_eq!(next["issue"]["id"], "I_139");
        assert_eq!(next["priority"]["name"], "Low");
        selected.push(json!({"issue":next["issue"],"item":next["item"],"priority":next["priority"],"cycle":next["cycle"],"fields":next["fields"],"reason":next["reason"]}));
    }
    assert!(selected.windows(2).all(|pair| pair[0] == pair[1]));
}

#[test]
fn bounds() {
    let fixture = Fixture::new("ordinary", 7);
    let one = fixture.success("next", Some(1), 7);
    assert_eq!(fixture.calls("items").len(), 7);
    assert_eq!(one["next"]["item"], "ITEM_4");
    let fixture = Fixture::new("ordinary", 7);
    assert_eq!(
        fixture.success("next", Some(100), 7)["next"]["item"],
        one["next"]["item"]
    );
    let fixture = Fixture::new("ordinary", 208);
    fixture.refused(Some(50), 4, "concord.project.incomplete");
    assert_eq!(fixture.calls("items").len(), 4);
    assert!(fixture.calls("confirm").is_empty());
}

#[test]
fn failures() {
    for (mode, code) in [
        ("failure", "concord.project.provider"),
        ("truncated", "concord.project.changed"),
        ("duplicate", "concord.project.changed"),
        ("cursor", "concord.project.provider"),
        ("metadata", "concord.project.changed"),
        ("selected", "concord.project.changed"),
        ("later", "concord.project.changed"),
        ("final", "concord.project.changed"),
    ] {
        let fixture = Fixture::new(mode, 208);
        fixture.refused(Some(50), 20, code);
        assert!(fixture.calls("items").len() <= 5, "{mode}");
    }
}

#[test]
fn complete() {
    let fixture = Fixture::new("ordinary", 208);
    let result = fixture.success("show", Some(100), 20);
    assert_eq!(result["project"]["complete"], true);
    assert_eq!(
        result["project"]["collections"]["items"]["nodes"]
            .as_array()
            .unwrap()
            .len(),
        208
    );
    assert_eq!(fixture.calls("items").len(), 11);
    assert!(fixture.calls("items").iter().all(|call| call["size"] == 20));
    assert_eq!(fixture.calls("fields")[0]["size"], 100);
}

pub(super) struct Fixture {
    root: tempfile::TempDir,
    server: Server,
    calls: Arc<Mutex<Vec<Value>>>,
}

impl Fixture {
    pub fn new(mode: &str, count: usize) -> Self {
        let root = tempfile::tempdir().unwrap();
        let calls = Arc::new(Mutex::new(Vec::new()));
        let shared = Arc::clone(&calls);
        let mode = mode.to_owned();
        let server = Server::start(move |request| handle(request, &mode, count, &shared));
        Self {
            root,
            server,
            calls,
        }
    }

    fn run(&self, operation: &str, size: Option<u16>, pages: u16) -> Output {
        let mut command = spawn::concord(self.root.path());
        self.server.attach(&mut command);
        command
            .args([
                "--home",
                "/deliberately-unavailable-estate",
                "project",
                operation,
                "PerishLab/1",
                "--json",
            ])
            .args(["--max-pages", &pages.to_string()]);
        if let Some(size) = size {
            command.args(["--page-size", &size.to_string()]);
        }
        command.output().unwrap()
    }

    pub fn success(&self, operation: &str, size: Option<u16>, pages: u16) -> Value {
        let output = self.run(operation, size, pages);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(!self.root.path().join("workspace").exists());
        serde_json::from_slice(&output.stdout).unwrap()
    }

    pub fn refused(&self, size: Option<u16>, pages: u16, code: &str) {
        let output = self.run("next", size, pages);
        assert!(!output.status.success() && output.stdout.is_empty());
        let error: Value = serde_json::from_slice(&output.stderr).unwrap();
        assert_eq!(error["error"]["code"], code);
    }

    pub fn calls(&self, kind: &str) -> Vec<Value> {
        self.calls
            .lock()
            .unwrap()
            .iter()
            .filter(|call| call["kind"] == kind)
            .cloned()
            .collect()
    }
}

fn handle(request: Request, mode: &str, count: usize, calls: &Arc<Mutex<Vec<Value>>>) -> Reply {
    assert_eq!(request.method, "POST");
    assert_eq!(request.path, "/graphql");
    let query = request.query();
    assert!(query.starts_with("query"));
    let variables = request.variables();
    let kind = classify(&query);
    let previous = {
        let mut log = calls.lock().unwrap();
        let prior = log.clone();
        log.push(json!({"kind":kind,"size":variables.get("size"),"after":variables.get("after")}));
        prior
    };
    let chosen = if count > 100 { 139 } else { 4 };
    match kind {
        "root" => root(),
        "confirm" => confirm(mode, chosen, &previous),
        "final" => finality(mode),
        _ => section(kind, &variables, mode, (count, chosen)),
    }
}

fn root() -> Reply {
    let organization = json!({"login":"PerishLab","projectV2":overview(&meta())});
    Reply::json(json!({"data":{"organization":organization}}))
}

fn confirm(mode: &str, chosen: usize, previous: &[Value]) -> Reply {
    let mut value = issue(chosen, chosen);
    let repeated = previous.iter().any(|call| call["kind"] == "confirm");
    if mode == "selected" || (mode == "later" && repeated) {
        value["updatedAt"] = "changed".into();
    }
    Reply::json(json!({"data":{"node":value}}))
}

fn finality(mode: &str) -> Reply {
    let mut project = meta();
    if mode == "final" {
        project["updatedAt"] = "changed".into();
    }
    Reply::json(json!({"data":{"node":overview(&project)}}))
}

fn classify(query: &str) -> &'static str {
    if query.contains("RunsealOverview") {
        return "root";
    }
    if query.contains("RunsealConfirm") {
        return "confirm";
    }
    if query.contains("RunsealMetadata") {
        return "final";
    }
    let names = [
        "fields",
        "items",
        "views",
        "workflows",
        "repositories",
        "statusUpdates",
    ];
    names
        .into_iter()
        .find(|name| query.contains(&format!("{name}(first:")))
        .unwrap_or("final")
}

fn section(kind: &str, variables: &Value, mode: &str, bounds: (usize, usize)) -> Reply {
    let (count, chosen) = bounds;
    let day = today();
    let nodes = match kind {
        "fields" => fields(&day),
        "items" => (0..count).map(|index| item(index, chosen, &day)).collect(),
        _ => Vec::new(),
    };
    let start: usize = variables["after"]
        .as_str()
        .and_then(|value| value.parse().ok())
        .unwrap_or(0);
    let size = variables["size"].as_u64().unwrap_or(50) as usize;
    let end = (start + size).min(nodes.len());
    let mut page = nodes[start..end].to_vec();
    let mut more = end < nodes.len();
    let mut cursor = more.then(|| end.to_string());
    let mut project = meta();
    if kind == "items" && start > 0 {
        match mode {
            "failure" => return Reply::status(500, json!({"message":"failed"})),
            "truncated" => more = false,
            "duplicate" => page[0]["id"] = "ITEM_0".into(),
            "cursor" => cursor = variables["after"].as_str().map(str::to_owned),
            "metadata" => project["updatedAt"] = "changed".into(),
            _ => {}
        }
    }
    let info = json!({"hasNextPage":more,"endCursor":cursor});
    project[kind] = json!({"totalCount":nodes.len(),"nodes":page,"pageInfo":info});
    Reply::json(json!({"data":{"node":project}}))
}
