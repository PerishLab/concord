mod mutate;
mod query;
mod schema;

use super::super::spawn::{self, Reply, Request, Server};
use super::planning;
use serde_json::{Value, json};
use std::process::Output;
use std::sync::{Arc, Mutex};

pub(super) struct Store(pub(super) Value);

pub(super) struct Fixture {
    root: tempfile::TempDir,
    state: Arc<Mutex<Store>>,
    server: Server,
}

impl Fixture {
    pub fn new(mode: &str) -> Self {
        let root = tempfile::tempdir().unwrap();
        let project = planning::meta();
        let initial = json!({"project":project,"projects":[project],"fields":[field("F_local","Planning",false),field("F_issue","Priority",true)],"items":[],"views":[],"workflows":[],"repositories":[],"statusUpdates":[],"issues":[issue("I_one"),issue("I_two")],"mode":mode,"writes":[],"clock":0});
        let state = Arc::new(Mutex::new(Store(initial)));
        let shared = Arc::clone(&state);
        let server = Server::start(move |request| shared.lock().unwrap().handle(request));
        Self {
            root,
            state,
            server,
        }
    }

    pub fn run(&self, operation: &str, input: &Value) -> Output {
        self.target(operation, "PerishLab/1", input)
    }

    pub fn target(&self, operation: &str, target: &str, input: &Value) -> Output {
        let path = self.root.path().join("input.json");
        std::fs::write(&path, input.to_string()).unwrap();
        let mut command = spawn::concord(self.root.path());
        self.server.attach(&mut command);
        command.current_dir(self.root.path()).args([
            "--home",
            "/unavailable-estate",
            "--json",
            "project",
            operation,
        ]);
        let input = ["--input", path.to_str().unwrap()];
        match operation {
            "prepare" => {
                command.arg(target).args(input);
            }
            "list" => {
                command.arg(target);
            }
            "apply" => {
                command.arg("--apply").args(input);
            }
            _ => {
                command.args(input);
            }
        }
        command.output().unwrap()
    }

    pub fn state(&self) -> Value {
        self.state.lock().unwrap().0.clone()
    }

    pub fn change(&self, mutate: impl FnOnce(&mut Value)) {
        let mut guard = self.state.lock().unwrap();
        mutate(&mut guard.0);
    }
}

impl Store {
    fn handle(&mut self, request: Request) -> Reply {
        let calls = self.0["calls"].as_u64().unwrap_or(0) + 1;
        self.0["calls"] = json!(calls);
        if request.method == "GET" && request.path.contains("/fields") {
            return Reply::json(self.columns());
        }
        if request.method == "POST" && request.path.contains("/views") {
            return self.view(&request.json());
        }
        if request.method != "POST" || request.path != "/graphql" {
            return Reply::status(500, json!({"message":"refused"}));
        }
        let query = request.query();
        let variables = request.variables();
        if query.starts_with("mutation") {
            return self.mutate(&query, &variables);
        }
        self.query(&query, &variables)
    }

    pub(super) fn columns(&self) -> Value {
        let number = self.0["project"]["number"].as_u64().unwrap_or(1);
        let url = format!("https://api.github.com/orgs/PerishLab/projectsV2/{number}");
        let mut nodes: Vec<Value> = self.0["fields"]
            .as_array()
            .unwrap()
            .iter()
            .enumerate()
            .map(|(index, entry)| {
                let id = match entry["id"].as_str().unwrap_or("") {
                    "F_local" => 100,
                    "F_issue" => 101,
                    "F_new" => 102,
                    _ => 200 + index as u64,
                };
                let kind = entry["dataType"].as_str().unwrap_or("").to_lowercase();
                json!({"id":id,"node_id":entry["id"],"name":entry["name"],"data_type":kind,"project_url":url})
            })
            .collect();
        nodes.push(json!({"id":199,"node_id":"F_type","name":"Type","data_type":"issue_type","project_url":url}));
        Value::Array(nodes)
    }

    pub(super) fn stamp(&mut self) {
        let clock = self.0["clock"].as_u64().unwrap_or(0) + 1;
        self.0["clock"] = json!(clock);
        self.0["project"]["updatedAt"] = json!(format!("stamp-{clock}"));
    }

    pub(super) fn projected(&self) -> Value {
        let mut value = self.0["project"].clone();
        if self.0["projection"] == true {
            for key in ["title", "shortDescription", "readme", "public"] {
                value[key] = self.0["previous"][key].clone();
            }
        }
        value
    }
}

pub(super) fn page(nodes: Vec<Value>) -> Value {
    let info = json!({"hasNextPage":false,"endCursor":null});
    json!({"nodes":nodes,"totalCount":nodes.len(),"pageInfo":info})
}

pub(super) fn overview(project: &Value) -> Value {
    planning::overview(project)
}

fn issue(id: &str) -> Value {
    let kind = json!({"id":"IT_task","name":"Task"});
    let repo = json!({"id":"R_repo","owner":{"id":"O_lab"}});
    json!({"__typename":"Issue","id":id,"number":4,"url":"https://github.com/PerishLab/concord/issues/4","state":"OPEN","updatedAt":"issue-original","issueType":kind,"repository":repo})
}

fn field(id: &str, name: &str, attached: bool) -> Value {
    let option = json!({"id":"old","name":"Candidate","color":"GRAY","description":""});
    let mut value = json!({"__typename":"ProjectV2SingleSelectField","id":id,"name":name,"dataType":"SINGLE_SELECT","isIssueField":attached,"updatedAt":"field-original","project":{"id":"PVT_plan"},"options":[option]});
    if attached {
        value["issueField"] = json!({"id":"IF_priority","fullDatabaseId":"42","name":name,"dataType":"SINGLE_SELECT"});
    }
    value
}

pub(super) fn intent(action: Value) -> Value {
    json!({"reason":"Review the domain candidate pool inside the entrusted planning boundary.","action":action})
}

pub(super) fn decoded(output: &Output) -> Value {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

pub(super) fn refused(output: &Output, code: &str) -> Value {
    assert!(
        !output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
    let error: Value = serde_json::from_slice(&output.stderr).unwrap();
    assert_eq!(error["error"]["code"], code, "{error}");
    error
}
