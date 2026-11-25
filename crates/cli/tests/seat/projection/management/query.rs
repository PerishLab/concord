use super::super::super::spawn::Reply;
use super::{Store, overview, page};
use serde_json::{Value, json};

impl Store {
    pub(super) fn query(&mut self, query: &str, variables: &Value) -> Reply {
        if query.contains("RunsealAccount") {
            let org = json!({"id":"O_lab","database":1,"login":"PerishLab","url":"https://github.com/PerishLab"});
            return Reply::json(json!({"data":{"organization":org}}));
        }
        if query.contains("RunsealOrigins") {
            let owner = self.0["origin_owner"].as_str().unwrap_or("O_lab");
            let fields = page(vec![self.0["origin"].clone()]);
            let org = json!({"id":owner,"login":"PerishLab","issueFields":fields});
            return Reply::json(json!({"data":{"organization":org}}));
        }
        if query.contains("RunsealProjects") {
            return self.projects();
        }
        if query.contains("RunsealOverview") {
            return self.start();
        }
        if query.contains("RunsealMetadata") {
            let node = overview(&self.projected());
            return Reply::json(json!({"data":{"node":node}}));
        }
        if query.contains("RunsealCandidates") {
            return self.candidates(variables);
        }
        if query.contains("RunsealExists") && variables["id"] == "IF_priority" {
            let node = json!({"id":"IF_priority","__typename":"IssueFieldSingleSelect"});
            return Reply::json(json!({"data":{"node":node}}));
        }
        if query.contains("RunsealSeat") && variables["id"] == "R_repo" {
            let owner = json!({"id":"O_lab"});
            let node = json!({"__typename":"Repository","id":"R_repo","nameWithOwner":"PerishLab/concord","owner":owner});
            return Reply::json(json!({"data":{"node":node}}));
        }
        self.board(query, variables)
    }

    fn projects(&self) -> Reply {
        let current = &self.0["project"];
        let rows: Vec<Value> = self.0["projects"]
            .as_array()
            .unwrap()
            .iter()
            .map(|entry| {
                let entry = if entry["id"] == current["id"] {
                    current
                } else {
                    entry
                };
                let owner = json!({"kind":"Organization","id":entry["owner"]["id"],"login":entry["owner"]["login"]});
                json!({
                    "id": entry["id"],
                    "kind": "ProjectV2",
                    "number": entry["number"],
                    "title": entry["title"],
                    "url": entry["url"],
                    "public": entry["public"],
                    "updated": entry["updatedAt"],
                    "owner": owner,
                })
            })
            .collect();
        let info = json!({"more":false,"cursor":null});
        let list = json!({"total":rows.len(),"rows":rows,"page":info});
        let org = json!({"id":"O_lab","login":"PerishLab","url":"https://github.com/PerishLab","kind":"Organization","list":list});
        Reply::json(json!({"data":{"organization":org}}))
    }

    fn start(&mut self) -> Reply {
        let starts = self.0["starts"].as_u64().unwrap_or(0) + 1;
        self.0["starts"] = json!(starts);
        let mode = self.0["mode"].as_str().unwrap_or("").to_owned();
        if starts == 2 && !self.alter(&mode) {
            return Reply::status(500, json!({"message":"refused"}));
        }
        let delay = self.0["delay"].as_u64().unwrap_or(0);
        let projection = mode == "delayed-project" && delay > 0;
        self.0["projection"] = json!(projection);
        if projection {
            self.0["delay"] = json!(delay - 1);
        }
        let value = overview(&self.projected());
        if mode == "prepare-key-order" && starts == 2 {
            return reversed(&value);
        }
        let org = json!({"login":"PerishLab","projectV2":value});
        Reply::json(json!({"data":{"organization":org}}))
    }

    fn alter(&mut self, mode: &str) -> bool {
        match mode {
            "prepare-project" => self.0["project"]["title"] = "DIAGNOSTIC_SECRET".into(),
            "prepare-items" => {
                let content = self.0["issues"][0].clone();
                let project = json!({"id":self.0["project"]["id"]});
                let item = json!({"id":"ITEM_DIAGNOSTIC_SECRET","type":"ISSUE","isArchived":false,"updatedAt":"DIAGNOSTIC_SECRET","project":project,"content":content});
                self.0["items"].as_array_mut().unwrap().push(item);
            }
            "prepare-issues" => self.0["issues"][0]["updatedAt"] = "DIAGNOSTIC_SECRET".into(),
            "prepare-fields" => self.0["fields"].as_array_mut().unwrap().reverse(),
            "prepare-failure" => return false,
            other if other.starts_with("prepare-history-") => {
                let entry = json!({"id":"DIAGNOSTIC_SECRET","body":format!("DIAGNOSTIC_SECRET {other}"),"createdAt":"DIAGNOSTIC_SECRET","updatedAt":"DIAGNOSTIC_SECRET"});
                self.0["statusUpdates"].as_array_mut().unwrap().push(entry);
            }
            _ => {}
        }
        true
    }

    fn candidates(&self, variables: &Value) -> Reply {
        let issues = self.0["issues"].as_array().unwrap();
        let nodes: Vec<Value> = variables["ids"]
            .as_array()
            .unwrap()
            .iter()
            .map(|identity| {
                issues
                    .iter()
                    .find(|entry| entry["id"] == *identity)
                    .cloned()
                    .unwrap_or(Value::Null)
            })
            .collect();
        Reply::json(json!({"data":{"nodes":nodes}}))
    }

    fn board(&mut self, query: &str, variables: &Value) -> Reply {
        for name in [
            "fields",
            "items",
            "views",
            "workflows",
            "repositories",
            "statusUpdates",
        ] {
            if !query.contains(&format!("{name}(first:")) {
                continue;
            }
            return self.section(name, variables);
        }
        Reply::status(500, json!({"message":"refused"}))
    }

    fn section(&mut self, name: &str, variables: &Value) -> Reply {
        let mode = self.0["mode"].as_str().unwrap_or("").to_owned();
        let starts = self.0["starts"].as_u64().unwrap_or(0);
        if name == "fields" && mode == "prepare-incomplete" && starts == 2 {
            let mut project = self.projected();
            project[name] = page(self.0[name].as_array().unwrap().clone());
            project[name]["pageInfo"]["hasNextPage"] = json!(true);
            return Reply::json(json!({"data":{"node":project}}));
        }
        let pending = self.0["snapshot"].as_u64().unwrap_or(0);
        if name == "items" && mode.starts_with("snapshot") && pending > 0 {
            return self.snapshot(&mode, variables, pending);
        }
        let Some(nodes) = self.observed(name) else {
            return Reply::status(500, json!({"message":"refused"}));
        };
        let mut project = self.projected();
        project[name] = page(nodes);
        Reply::json(json!({"data":{"node":project}}))
    }

    fn snapshot(&mut self, mode: &str, variables: &Value, pending: u64) -> Reply {
        let mut project = self.projected();
        if variables["after"].is_null() {
            let owner = json!({"id":self.0["project"]["id"]});
            let content = self.0["issues"].as_array().unwrap().last().unwrap().clone();
            let phantom = json!({"id":"ITEM_phantom","type":"ISSUE","isArchived":false,"updatedAt":"old","project":owner,"content":content});
            let info = json!({"hasNextPage":true,"endCursor":"snapshot"});
            project["items"] = json!({"nodes":[phantom],"totalCount":2,"pageInfo":info});
            return Reply::json(json!({"data":{"node":project}}));
        }
        if variables["after"] != "snapshot" {
            return Reply::status(500, json!({"message":"refused"}));
        }
        self.0["snapshot"] = json!(pending - 1);
        let conflicts = self.0["conflicts"].as_u64().unwrap_or(0) + 1;
        self.0["conflicts"] = json!(conflicts);
        if mode == "snapshot-context" {
            let entry = json!({"id":"U_foreign","body":"A human changes the planning boundary.","createdAt":"new","updatedAt":"new"});
            self.0["statusUpdates"].as_array_mut().unwrap().push(entry);
        }
        project["items"] = page(self.0["items"].as_array().unwrap().clone());
        Reply::json(json!({"data":{"node":project}}))
    }

    fn observed(&mut self, name: &str) -> Option<Vec<Value>> {
        let mut nodes = self.0[name].as_array().unwrap().clone();
        let hidden = self.0["hidden"].as_str().map(str::to_owned);
        let mode = self.0["mode"].as_str().unwrap_or("").to_owned();
        if name != "items" || hidden.is_none() || mode == "ordinary" {
            return Some(nodes);
        }
        let observations = self.0["observations"].as_u64().unwrap_or(0) + 1;
        self.0["observations"] = json!(observations);
        if mode == "reader-failure" {
            return None;
        }
        let delay = self.0["delay"].as_u64().unwrap_or(0);
        if delay > 0 {
            self.hide(&mode, &hidden.unwrap(), &mut nodes);
            self.0["delay"] = json!(delay - 1);
            if mode == "pending-context" {
                let entry = json!({"id":"U_foreign","body":"Human changes the investment boundary.","createdAt":"new","updatedAt":"new"});
                self.0["statusUpdates"].as_array_mut().unwrap().push(entry);
            }
        }
        Some(nodes)
    }

    fn hide(&self, mode: &str, hidden: &str, nodes: &mut Vec<Value>) {
        if mode == "delayed-value" {
            for item in nodes {
                if item["id"] == hidden {
                    item["fieldValues"] = self.0["previous"].clone();
                }
            }
        } else {
            nodes.retain(|item| item["content"]["id"] != hidden);
        }
    }
}

fn reversed(value: &Value) -> Reply {
    let entries: Vec<String> = value
        .as_object()
        .unwrap()
        .iter()
        .rev()
        .map(|(key, item)| format!("{}:{item}", json!(key)))
        .collect();
    let body = format!(
        "{{\"data\":{{\"organization\":{{\"login\":\"PerishLab\",\"projectV2\":{{{}}}}}}}}}",
        entries.join(",")
    );
    Reply::raw(200, body.into_bytes())
}
