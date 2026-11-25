use super::super::{evaluate, world};
use crate::spawn::{Reply, Request};
use serde_json::{Value, json};
use std::sync::Mutex;

pub(super) struct State {
    pub mode: String,
    closed: bool,
    attempts: u64,
    heads: u64,
    calls: u64,
}

impl State {
    pub fn new() -> Self {
        Self {
            mode: "ok".into(),
            closed: false,
            attempts: 0,
            heads: 0,
            calls: 0,
        }
    }

    pub fn json(&self) -> Value {
        json!({
            "mode": self.mode,
            "closed": self.closed,
            "attempts": self.attempts,
            "heads": self.heads,
            "calls": self.calls,
        })
    }

    fn close(&mut self, query: &str, variables: &Value) -> Reply {
        self.attempts += 1;
        match self.mode.as_str() {
            "fail-before" => return Reply::status(500, json!({"message": "fail-before"})),
            "forbidden" => return error("forbidden"),
            _ => {}
        }
        assert_eq!(*variables, json!({"issue": "I_issue"}));
        assert!(query.contains("stateReason: COMPLETED"));
        self.closed = true;
        if self.mode == "lost" {
            return Reply::status(500, json!({"message": "lost"}));
        }
        let id = if self.mode == "ack" {
            "wrong"
        } else {
            "I_issue"
        };
        let issue = json!({
            "id": id,
            "state": "CLOSED",
            "reason": "COMPLETED",
            "repository": {"id": "R_concord"},
        });
        Reply::json(json!({"data": {"closeIssue": {"issue": issue}}}))
    }

    fn head(&mut self, data: &evaluate::World) -> Reply {
        self.heads += 1;
        match self.mode.as_str() {
            "readback" if self.closed => return Reply::status(500, json!({"message": "readback"})),
            "partial" => return error("partial"),
            _ => {}
        }
        let mut value = data.page["data"]["repository"]["issue"].clone();
        self.patch(&mut value);
        value["writable"] = json!(self.mode != "permission");
        value["reason"] = if self.closed || self.mode == "reason" {
            json!("COMPLETED")
        } else {
            Value::Null
        };
        self.mutate(&mut value);
        let repository = json!({"nameWithOwner": "PerishLab/concord", "issue": value});
        Reply::json(json!({"data": {"repository": repository}}))
    }

    fn mutate(&self, value: &mut Value) {
        match self.mode.as_str() {
            "auto" => value["kind"]["name"] = "Auto".into(),
            "unknown" => value["kind"]["name"] = "Ghost".into(),
            "head-drift" if self.heads > 1 => value["updated"] = "2026-10-03T00:00:00Z".into(),
            "node" => value["id"] = "I_foreign".into(),
            "coordinate" => value["url"] = "https://github.com/PerishLab/concord/issues/84".into(),
            "repository" => value["repository"]["id"] = "R_foreign".into(),
            _ => {}
        }
    }

    fn relations(&self, data: &evaluate::World) -> Value {
        let mut reply = data.relations.clone();
        let key = match self.mode.as_str() {
            "child" => Some("children"),
            "blocked" => Some("blockers"),
            "after-unmet" if self.closed => Some("blockers"),
            _ => None,
        };
        if let Some(key) = key {
            let blocker = json!({"id": "I_blocker", "state": "OPEN"});
            reply["data"]["repository"]["issue"][key] = world::connection(vec![blocker], None, 1);
        }
        reply
    }

    fn freshness(&self, data: &evaluate::World) -> Value {
        let mut reply = data.fresh.clone();
        match self.mode.as_str() {
            "unordered" => {
                reply["data"]["repository"]["issue"]["edited"] = "2026-09-30T00:00:00Z".into();
            }
            "after-unknown" if self.closed => {
                reply["data"]["repository"]["issue"]["edited"] = "2026-09-30T00:00:00Z".into();
            }
            "reopen" => {
                let event = json!({"id": "RE_new", "created": "2026-10-01T00:00:00Z"});
                reply["data"]["repository"]["issue"]["list"] =
                    world::connection(vec![event], None, 1);
            }
            "closure" => append(
                &mut reply["data"]["closure"]["body"],
                "\nNew attestation prose",
            ),
            "declaration" => append(
                &mut reply["data"]["declaration"]["body"],
                "\nNew promise prose",
            ),
            _ => {}
        }
        reply
    }

    fn page(&self, data: &evaluate::World) -> Value {
        let mut reply = data.page.clone();
        let value = &mut reply["data"]["repository"]["issue"];
        match self.mode.as_str() {
            "closure" => append(
                &mut value["comments"]["rows"][1]["body"],
                "\nNew attestation prose",
            ),
            "declaration" => append(
                &mut value["comments"]["rows"][0]["body"],
                "\nNew promise prose",
            ),
            "needs" => {
                value["labels"]["rows"]
                    .as_array_mut()
                    .unwrap()
                    .push(json!({"name": "needs:revalidation"}));
                let total = value["labels"]["total"].as_u64().unwrap() + 1;
                value["labels"]["total"] = total.into();
            }
            "legacy" => {
                value["comments"] = world::connection(vec![], None, 0);
                value["labels"] = world::connection(vec![], None, 0);
            }
            _ => {}
        }
        reply
    }

    fn patch(&self, value: &mut Value) {
        if self.closed {
            value["state"] = "CLOSED".into();
        }
        if self.closed || matches!(self.mode.as_str(), "stamp" | "reopen") {
            value["updated"] = "2026-10-02T00:00:00Z".into();
        }
        if self.mode == "body" || (self.mode == "body-after" && self.closed) {
            append(&mut value["body"], "\nChanged promised result\n");
        }
    }
}

pub(super) fn handle(
    request: Request,
    state: &Mutex<State>,
    data: &Mutex<evaluate::World>,
) -> Reply {
    let query = request.query();
    let variables = request.variables();
    let mut state = state.lock().unwrap();
    let data = data.lock().unwrap();
    state.calls += 1;
    if query.starts_with("mutation RunsealClose") {
        return state.close(&query, &variables);
    }
    if query.starts_with("mutation") {
        return Reply::status(500, json!({"message": "unexpected mutation"}));
    }
    if query.starts_with("query RunsealIssue") {
        return state.head(&data);
    }
    if query.contains("object(expression:") {
        let repository = json!({"id": "R_concord", "object": null});
        return Reply::json(json!({"data": {"repository": repository}}));
    }
    let mut reply = if query.contains("children:subIssues") {
        state.relations(&data)
    } else if query.contains("timelineItems") {
        state.freshness(&data)
    } else {
        state.page(&data)
    };
    if let Some(target) = reply.pointer_mut("/data/repository/issue") {
        state.patch(target);
    }
    Reply::json(reply)
}

fn error(message: &str) -> Reply {
    let errors = json!([{"message": message}]);
    Reply::json(json!({"data": null, "errors": errors}))
}

fn append(field: &mut Value, extra: &str) {
    let text = format!("{}{extra}", field.as_str().unwrap());
    *field = text.into();
}
