use super::super::super::spawn::{self, Reply, Request, Server};
use super::super::planning::overview;
use super::paging::today;
use serde_json::{Value, json};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Output;

struct Mock {
    mode: String,
    calls: PathBuf,
}

pub(super) fn run(root: &Path, mode: &str, extra: &[&str]) -> Output {
    let mock = Mock {
        mode: mode.to_owned(),
        calls: root.join("calls"),
    };
    let server = Server::start(move |request| mock.handle(request));
    let mut command = spawn::concord(root);
    server.attach(&mut command);
    command
        .args([
            "--home",
            "/missing-estate",
            "--json",
            "project",
            "survey",
            "PerishLab",
            "--project",
            "PerishLab/1",
            "--project",
            "PerishLab/2",
        ])
        .args(extra)
        .output()
        .unwrap()
}

impl Mock {
    fn handle(&self, request: Request) -> Reply {
        let query = request.query();
        let variables = request.variables();
        let mut log = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.calls)
            .unwrap();
        let _ = writeln!(log, "{query}");
        assert!(!query.trim_start().starts_with("mutation"));
        if self.mode == "permission" {
            return Reply::status(403, json!({"message":"forbidden"}));
        }
        let after = variables["after"].as_str();
        if query.contains("RunsealCatalog") {
            return self.catalog(after);
        }
        if query.contains("RunsealOpen") {
            return self.backlog(&variables, after);
        }
        if query.contains("RunsealOverview") {
            let number = variables["number"].as_u64().unwrap();
            let mut result = project(number);
            if self.mode == "project-foreign" {
                result["owner"]["id"] = "foreign".into();
            }
            let organization = json!({"login":"PerishLab","projectV2":overview(&result)});
            return Reply::json(json!({"data":{"organization":organization}}));
        }
        let number: u64 = variables["id"].as_str().unwrap()[1..].parse().unwrap();
        if query.contains("RunsealMetadata") {
            let mut result = project(number);
            if self.mode == "changed" {
                result["updatedAt"] = "new".into();
            }
            let data = json!({"node":overview(&result)});
            return Reply::json(json!({"data":data}));
        }
        self.board(&query, number)
    }

    fn catalog(&self, after: Option<&str>) -> Reply {
        let mut nodes = vec![repository(1)];
        match self.mode.as_str() {
            "foreign" => nodes[0]["owner"]["id"] = "foreign".into(),
            "pages" if after.is_some() => nodes = vec![repository(2)],
            _ => {}
        }
        let more = self.mode == "pages" && after.is_none();
        let total = (self.mode == "pages").then_some(2);
        let value = json!({"id":"O","login":"PerishLab","url":"https://github.com/PerishLab","repositories":connection(nodes,more,total)});
        Reply::json(json!({"data":{"organization":value}}))
    }

    fn backlog(&self, variables: &Value, after: Option<&str>) -> Reply {
        let number: u64 = variables["id"].as_str().unwrap()[1..].parse().unwrap();
        let mut repo = repository(number);
        let mut nodes = if number == 1 {
            vec![
                issue(1, Some("Task")),
                issue(2, Some("Task")),
                issue(3, Some("Auto")),
                issue(4, None),
            ]
        } else {
            Vec::new()
        };
        let mut more = false;
        let mut total = None;
        match self.mode.as_str() {
            "pages" if number == 1 => {
                total = Some(4);
                nodes = if after.is_some() {
                    nodes[1..].to_vec()
                } else {
                    nodes[..1].to_vec()
                };
                more = after.is_none();
            }
            "repeat" => nodes.push(nodes[0].clone()),
            "issue-foreign" => nodes[0]["repository"]["id"] = "foreign".into(),
            _ => {}
        }
        repo["issues"] = connection(nodes, more, total);
        Reply::json(json!({"data":{"node":repo}}))
    }

    fn board(&self, query: &str, number: u64) -> Reply {
        let mut result = project(number);
        if query.contains("fields(first:") {
            let nodes = self.fields(number);
            result["fields"] = if self.mode == "field-page" {
                connection(nodes[..1].to_vec(), true, Some(3))
            } else {
                connection(nodes, false, None)
            };
        } else if query.contains("items(first:") {
            assert!(query.contains("ProjectV2ItemFieldTextValue{text field"));
            let mut rows = vec![self.item(number)];
            match self.mode.as_str() {
                "changed-issue" => rows[0]["content"]["updatedAt"] = "new".into(),
                "item-foreign" => rows[0]["project"]["id"] = "foreign".into(),
                _ => {}
            }
            result["items"] = if self.mode == "item-page" {
                connection(rows, true, Some(2))
            } else {
                connection(rows, false, None)
            };
        }
        Reply::json(json!({"data":{"node":result}}))
    }

    fn fields(&self, number: u64) -> Vec<Value> {
        let root = json!({"id":format!("P{number}")});
        let options: Vec<Value> = ["Urgent", "High", "Medium", "Low"]
            .into_iter()
            .map(|name| json!({"id":format!("{name}{number}"),"name":name}))
            .collect();
        let priority = json!({"__typename":"ProjectV2SingleSelectField","id":format!("F_priority{number}"),"name":"Priority","dataType":"SINGLE_SELECT","isIssueField":false,"project":root,"options":options});
        let mut periods = Vec::new();
        if matches!(self.mode.as_str(), "ready" | "ambiguous-cycle") {
            let date = today();
            periods.push(json!({"id":"C","title":"Current","startDate":date,"duration":7}));
            if self.mode == "ambiguous-cycle" {
                let mut extra = periods[0].clone();
                extra["id"] = "D".into();
                periods.push(extra);
            }
        }
        let config =
            json!({"duration":7,"startDay":1,"iterations":periods,"completedIterations":[]});
        let cycle = json!({"__typename":"ProjectV2IterationField","id":format!("F_cycle{number}"),"name":"Cycle","dataType":"ITERATION","isIssueField":false,"project":root,"configuration":config});
        let text = json!({"__typename":"ProjectV2Field","id":format!("F_contribution{number}"),"name":"Contribution","dataType":"TEXT","isIssueField":false,"project":root});
        if self.mode == "missing-fields" {
            return vec![text];
        }
        let mut result = vec![priority, cycle.clone(), text];
        if self.mode == "ambiguous-fields" {
            let mut extra = cycle;
            extra["id"] = "another".into();
            result.push(extra);
        }
        result
    }

    fn item(&self, number: u64) -> Value {
        let field = json!({"id":format!("F_contribution{number}")});
        let value = json!({"__typename":"ProjectV2ItemFieldTextValue","field":field,"text":format!("Different benefit {number}")});
        let mut values = connection(vec![value], false, None);
        if self.mode == "nested" {
            values["pageInfo"] = json!({"hasNextPage":true,"endCursor":"next"});
        }
        let content = if self.mode == "unknown-content" {
            Value::Null
        } else {
            let mut value = issue(1, Some("Task"));
            value["__typename"] = "Issue".into();
            value
        };
        let owner = json!({"id":format!("P{number}")});
        json!({"id":format!("ITEM{number}"),"type":"ISSUE","isArchived":false,"updatedAt":"same","project":owner,"content":content,"fieldValues":values})
    }
}

fn connection(nodes: Vec<Value>, more: bool, total: Option<usize>) -> Value {
    let count = total.unwrap_or(nodes.len());
    let cursor = more.then_some("next");
    let info = json!({"hasNextPage":more,"endCursor":cursor});
    json!({"nodes":nodes,"totalCount":count,"pageInfo":info})
}

fn repository(number: u64) -> Value {
    let owner = json!({"id":"O","login":"PerishLab"});
    json!({"id":format!("R{number}"),"nameWithOwner":format!("PerishLab/r{number}"),"url":format!("https://github.com/PerishLab/r{number}"),"owner":owner,"isArchived":false,"hasIssuesEnabled":true})
}

fn issue(number: u64, kind: Option<&str>) -> Value {
    let kind = kind.map(|name| json!({"id":"T","name":name}));
    let owner = json!({"id":"O","login":"PerishLab"});
    let repo = json!({"id":"R1","nameWithOwner":"PerishLab/r1","owner":owner});
    json!({"id":format!("I{number}"),"number":number,"title":format!("Result {number}"),"url":format!("https://github.com/PerishLab/r1/issues/{number}"),"state":"OPEN","updatedAt":"same","issueType":kind,"repository":repo})
}

fn project(number: u64) -> Value {
    let owner = json!({"__typename":"Organization","id":"O","login":"PerishLab"});
    json!({"__typename":"ProjectV2","id":format!("P{number}"),"number":number,"title":format!("Goal {number}"),"url":format!("https://github.com/orgs/PerishLab/projects/{number}"),"updatedAt":"same","closed":false,"public":false,"readme":"Goal, evidence and gap","shortDescription":"Goal","viewerCanUpdate":true,"owner":owner})
}
