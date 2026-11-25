use super::super::super::spawn::Reply;
use super::{Store, overview, page};
use serde_json::{Value, json};
use std::collections::BTreeMap;

impl Store {
    pub(super) fn view(&mut self, payload: &Value) -> Reply {
        let mapping: BTreeMap<u64, String> = self
            .columns()
            .as_array()
            .unwrap()
            .iter()
            .map(|entry| {
                (
                    entry["id"].as_u64().unwrap(),
                    entry["node_id"].as_str().unwrap().to_owned(),
                )
            })
            .collect();
        let refs = |key: &str| -> Value {
            let nodes = payload[key]
                .as_array()
                .into_iter()
                .flatten()
                .map(|id| json!({"id":mapping[&id.as_u64().unwrap()]}))
                .collect();
            page(nodes)
        };
        let mut visible = refs("visible_fields");
        visible.as_object_mut().unwrap().remove("totalCount");
        let sort: Vec<Value> = payload["sort_by"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|entry| {
                let id = entry[0].as_u64().unwrap();
                let direction = entry[1].as_str().unwrap().to_uppercase();
                json!({"direction":direction,"field":{"id":mapping[&id]}})
            })
            .collect();
        let count = self.0["views"].as_array().unwrap().len();
        let id = format!("V_{count}");
        let number = count + 1;
        let layout = payload["layout"].as_str().unwrap();
        let config = json!({"visibleFields": visible});
        let project = json!({"id": self.0["project"]["id"]});
        let record = json!({
            "id": id,
            "project": project,
            "number": number,
            "fullDatabaseId": "501",
            "name": payload["name"],
            "layout": format!("{}_LAYOUT", layout.to_uppercase()),
            "filter": payload["filter"],
            "updatedAt": "view-original",
            "configuration": config,
            "groupByFields": refs("group_by"),
            "verticalGroupByFields": refs("vertical_group_by"),
            "sortByFields": page(sort),
        });
        self.0["views"].as_array_mut().unwrap().push(record);
        self.0["writes"]
            .as_array_mut()
            .unwrap()
            .push("rest:view".into());
        self.stamp();
        let url = format!(
            "https://api.github.com/orgs/PerishLab/projectsV2/{}",
            self.0["project"]["number"]
        );
        let value = json!({"id":501,"number":number,"node_id":id,"name":payload["name"],"layout":layout,"project_url":url});
        Reply::status(201, json!({"value":value}))
    }

    pub(super) fn mutate(&mut self, query: &str, variables: &Value) -> Reply {
        let Some(name) = operation(query) else {
            return Reply::status(500, json!({"message":"refused"}));
        };
        let input = &variables["input"];
        self.0["writes"].as_array_mut().unwrap().push(json!(name));
        let response = match name {
            "createProjectV2StatusUpdate" => self.status(input),
            "addProjectV2ItemById" => self.add(input),
            "deleteProjectV2Item" => self.delete(input),
            "updateProjectV2" => self.configure(input),
            "createProjectV2Field" | "updateProjectV2Field" => self.schema(name, input),
            "updateProjectV2ItemFieldValue" | "clearProjectV2ItemFieldValue" => self.cell(input),
            "updateProjectV2ItemPosition" => self.position(input),
            "createProjectV2IssueField" => self.attach(input),
            "deleteProjectV2Field" => self.remove(input),
            "deleteProjectV2Workflow" => self.workflow(input),
            "deleteProjectV2View" => self.unview(input),
            "updateProjectV2View" => self.revise(input),
            "createProjectV2" => self.create(input),
            "linkProjectV2ToRepository" | "unlinkProjectV2FromRepository" => self.link(name, input),
            _ => None,
        };
        let Some(change) = response else {
            return Reply::status(500, json!({"message":"refused"}));
        };
        self.stamp();
        Reply::json(json!({"data":{"change":change}}))
    }

    fn status(&mut self, input: &Value) -> Option<Value> {
        let body = input["body"].as_str()?;
        let mode = self.0["mode"].as_str().unwrap_or("").to_owned();
        if mode == "record-failure" && body.contains("confirmed-native-state") {
            return None;
        }
        let id = format!("U_{}", self.0["statusUpdates"].as_array().unwrap().len());
        let entry = json!({"id":id,"body":body,"createdAt":"original","updatedAt":"original"});
        self.0["statusUpdates"].as_array_mut().unwrap().push(entry);
        if mode == "request-context" {
            let foreign = json!({"id":"U_foreign","body":"Human changes the investment boundary.","createdAt":"new","updatedAt":"new"});
            self.0["statusUpdates"]
                .as_array_mut()
                .unwrap()
                .push(foreign);
        }
        let project = json!({"id":self.0["project"]["id"]});
        let update = json!({"id":id,"body":body,"created":"original","updated":"original","start":null,"target":null,"status":null,"project":project});
        Some(json!({"statusUpdate":update}))
    }

    fn add(&mut self, input: &Value) -> Option<Value> {
        let mode = self.0["mode"].as_str().unwrap_or("").to_owned();
        let content = input["contentId"].as_str()?;
        if mode == "partial" && content == "I_two" {
            return None;
        }
        let issue = self.0["issues"]
            .as_array()
            .unwrap()
            .iter()
            .find(|entry| entry["id"] == content)?
            .clone();
        let owner = json!({"id":self.0["project"]["id"]});
        let item = format!("ITEM_{content}");
        let entry = json!({"id":item,"type":"ISSUE","isArchived":false,"updatedAt":"item-original","project":owner,"content":issue});
        self.0["items"].as_array_mut().unwrap().push(entry);
        match mode.as_str() {
            "snapshot-persistent" => self.0["snapshot"] = json!(999),
            value if value.starts_with("snapshot") => self.0["snapshot"] = json!(1),
            "delayed" | "invisible" | "pending-context" | "reader-failure" => {
                self.0["hidden"] = json!(content);
                self.0["delay"] = json!(if mode == "invisible" { 999 } else { 1 });
            }
            "changed-type" => self.0["issues"][1]["issueType"]["name"] = "Auto".into(),
            "context" => {
                let foreign = json!({"id":"U_foreign","body":"Human changes the investment boundary.","createdAt":"new","updatedAt":"new"});
                self.0["statusUpdates"]
                    .as_array_mut()
                    .unwrap()
                    .push(foreign);
            }
            "lost" => {
                self.stamp();
                return None;
            }
            _ => {}
        }
        let found = json!({"id":item,"project":owner,"content":{"id":content}});
        Some(json!({"item":found}))
    }

    fn delete(&mut self, input: &Value) -> Option<Value> {
        let id = input["itemId"].as_str()?;
        self.0["items"]
            .as_array_mut()
            .unwrap()
            .retain(|entry| entry["id"] != id);
        Some(json!({"deletedItemId":id}))
    }

    fn configure(&mut self, input: &Value) -> Option<Value> {
        if self.0["mode"] == "delayed-project" {
            self.0["previous"] = self.0["project"].clone();
            self.0["delay"] = json!(1);
        }
        for (key, value) in input.as_object()? {
            if key != "projectId" {
                self.0["project"][key] = value.clone();
            }
        }
        Some(json!({"project":overview(&self.0["project"])}))
    }

    fn cell(&mut self, input: &Value) -> Option<Value> {
        assert_eq!(input["projectId"], self.0["project"]["id"]);
        let mode = self.0["mode"].as_str().unwrap_or("").to_owned();
        let item = input["itemId"].as_str()?;
        let field = input["fieldId"].as_str()?;
        let index = self.0["items"]
            .as_array()
            .unwrap()
            .iter()
            .position(|entry| entry["id"] == item)?;
        let previous = self.0["items"][index]["fieldValues"].clone();
        let mut nodes: Vec<Value> = previous["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|entry| entry["field"]["id"] != field)
            .cloned()
            .collect();
        if mode == "delayed-value" {
            self.0["hidden"] = json!(item);
            self.0["delay"] = json!(1);
            self.0["previous"] = previous;
        }
        if let Some(data) = input.get("value") {
            nodes.push(entry(field, data));
        }
        self.0["items"][index]["fieldValues"] = page(nodes);
        if mode == "lost-value" {
            self.stamp();
            return None;
        }
        let owner = json!({"id":self.0["project"]["id"]});
        let found = json!({"id":item,"project":owner});
        Some(json!({"item":found}))
    }

    fn position(&mut self, input: &Value) -> Option<Value> {
        let id = input["itemId"].as_str()?;
        let items = self.0["items"].as_array_mut().unwrap();
        let found = items.iter().position(|entry| entry["id"] == id)?;
        let item = items.remove(found);
        let target = input["afterId"]
            .as_str()
            .and_then(|after| items.iter().position(|entry| entry["id"] == after))
            .map(|index| index + 1)
            .unwrap_or(0);
        items.insert(target, item);
        let total = json!({"totalCount":items.len()});
        Some(json!({"items":total}))
    }

    fn create(&mut self, input: &Value) -> Option<Value> {
        let mut value = self.0["project"].clone();
        value["id"] = "PVT_new".into();
        value["number"] = json!(2);
        value["title"] = input["title"].clone();
        value["url"] = "https://github.com/orgs/PerishLab/projects/2".into();
        self.0["project"] = value.clone();
        self.0["projects"]
            .as_array_mut()
            .unwrap()
            .push(value.clone());
        for key in ["fields", "views", "items", "statusUpdates"] {
            self.0[key] = json!([]);
        }
        let project = json!({"id":"PVT_new"});
        let workflow = json!({"id":"WF_default","name":"Default automation","enabled":true,"updatedAt":"original","project":project});
        self.0["workflows"] = json!([workflow]);
        Some(json!({"project":overview(&value)}))
    }

    fn link(&mut self, name: &str, input: &Value) -> Option<Value> {
        self.0["repositories"] = if name.starts_with("link") {
            let owner = json!({"id":"O_lab"});
            let repo = json!({"id":"R_repo","nameWithOwner":"PerishLab/concord","url":"https://github.com/PerishLab/concord","owner":owner});
            json!([repo])
        } else {
            json!([])
        };
        let repo = json!({"id":input["repositoryId"]});
        Some(json!({"repository":repo}))
    }
}

fn operation(query: &str) -> Option<&str> {
    let rest = query.split("change:").nth(1)?;
    rest.split('(').next()
}

fn entry(field: &str, data: &Value) -> Value {
    let target = json!({"id":field});
    if let Some(option) = data.get("singleSelectOptionId") {
        return json!({"__typename":"ProjectV2ItemFieldSingleSelectValue","optionId":option,"field":target});
    }
    if let Some(text) = data.get("text") {
        return json!({"__typename":"ProjectV2ItemFieldTextValue","text":text,"field":target});
    }
    json!({"__typename":"ProjectV2ItemFieldIterationValue","iterationId":data["iterationId"],"field":target})
}
