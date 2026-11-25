use super::Store;
use serde_json::{Value, json};

impl Store {
    pub(super) fn schema(&mut self, name: &str, input: &Value) -> Option<Value> {
        let project = self.0["project"]["id"].clone();
        let owner = json!({"id":project});
        let index = if name == "createProjectV2Field" {
            let value = json!({"__typename":"ProjectV2SingleSelectField","id":"F_new","name":input["name"],"dataType":input["dataType"],"isIssueField":false,"updatedAt":"field-original","project":owner,"options":[]});
            let fields = self.0["fields"].as_array_mut().unwrap();
            fields.push(value);
            fields.len() - 1
        } else {
            let id = input["fieldId"].as_str()?;
            let fields = self.0["fields"].as_array_mut().unwrap();
            let index = fields.iter().position(|entry| entry["id"] == id)?;
            if let Some(title) = input.get("name") {
                fields[index]["name"] = title.clone();
            }
            index
        };
        let field = &mut self.0["fields"][index];
        if let Some(options) = input.get("singleSelectOptions") {
            choices(field, options)?;
        }
        if let Some(calendar) = input.get("iterationConfiguration") {
            iterations(field, calendar);
        }
        let descriptor = json!({"id":field["id"],"name":field["name"],"issue":false,"project":owner,"origin":null});
        Some(json!({"field":descriptor}))
    }

    pub(super) fn attach(&mut self, input: &Value) -> Option<Value> {
        let origin = self.0["origin"].clone();
        let project = self.0["project"]["id"].clone();
        assert_eq!(input["projectId"], project);
        assert_eq!(input["issueFieldId"], origin["id"]);
        let owner = json!({"id":project});
        let entry = json!({"__typename":"ProjectV2SingleSelectField","id":"F_attached","name":origin["name"],"dataType":"SINGLE_SELECT","isIssueField":true,"updatedAt":"attached","project":owner,"issueField":origin,"options":origin["options"]});
        self.0["fields"].as_array_mut().unwrap().push(entry);
        if self.0["mode"] == "attachment-unknown" {
            self.stamp();
            return None;
        }
        let source = json!({"id":origin["id"]});
        let descriptor = json!({"id":"F_attached","name":origin["name"],"issue":true,"project":owner,"origin":source});
        Some(json!({"field":descriptor}))
    }

    pub(super) fn remove(&mut self, input: &Value) -> Option<Value> {
        let id = input["fieldId"].as_str()?;
        self.0["fields"]
            .as_array_mut()
            .unwrap()
            .retain(|entry| entry["id"] != id);
        let field = json!({"id":id});
        Some(json!({"field":field}))
    }

    pub(super) fn workflow(&mut self, input: &Value) -> Option<Value> {
        let id = input["workflowId"].as_str()?;
        self.0["workflows"]
            .as_array_mut()
            .unwrap()
            .retain(|entry| entry["id"] != id);
        let project = json!({"id":self.0["project"]["id"]});
        Some(json!({"id":id,"project":project}))
    }

    pub(super) fn unview(&mut self, input: &Value) -> Option<Value> {
        let id = input["viewId"].as_str()?;
        self.0["views"]
            .as_array_mut()
            .unwrap()
            .retain(|entry| entry["id"] != id);
        let view = json!({"id":id});
        Some(json!({"projectV2View":view}))
    }

    pub(super) fn revise(&mut self, input: &Value) -> Option<Value> {
        let id = input["viewId"].as_str()?;
        let view = self.0["views"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|entry| entry["id"] == id)?;
        for key in ["name", "filter", "layout"] {
            if let Some(value) = input.get(key) {
                view[key] = value.clone();
            }
        }
        if let Some(ids) = input
            .pointer("/configuration/visibleFieldIds")
            .and_then(Value::as_array)
        {
            let nodes: Vec<Value> = ids.iter().map(|entry| json!({"id":entry})).collect();
            let info = json!({"hasNextPage":false,"endCursor":null});
            view["configuration"]["visibleFields"] = json!({"nodes":nodes,"pageInfo":info});
        }
        let found = json!({"id":id});
        Some(json!({"projectV2View":found}))
    }
}

fn choices(field: &mut Value, options: &Value) -> Option<()> {
    let mut list = options.as_array()?.clone();
    for (position, option) in list.iter_mut().enumerate() {
        if option.get("id").is_none_or(Value::is_null) {
            option["id"] = json!(format!("choice-{position}"));
        }
    }
    field["options"] = Value::Array(list);
    Some(())
}

fn iterations(field: &mut Value, calendar: &Value) {
    field["__typename"] = "ProjectV2IterationField".into();
    field["dataType"] = "ITERATION".into();
    let mut periods = calendar["iterations"].as_array().unwrap().clone();
    for (position, period) in periods.iter_mut().enumerate() {
        if period.get("id").is_none_or(Value::is_null) {
            period["id"] = json!(format!("period-new-{position}"));
        }
    }
    let current: Vec<Value> = periods
        .iter()
        .filter(|entry| entry["startDate"].as_str().unwrap_or("") >= "2026-10-21")
        .cloned()
        .collect();
    let completed: Vec<Value> = periods
        .into_iter()
        .filter(|entry| entry["startDate"].as_str().unwrap_or("") < "2026-10-21")
        .collect();
    field["configuration"] = json!({"duration":calendar["duration"],"startDay":3,"iterations":current,"completedIterations":completed});
}
