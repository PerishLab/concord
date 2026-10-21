use super::model::{Action, Choice, Intent};
use serde_json::{Value, json};

pub(super) fn result(state: &Value, intent: &Intent) -> Option<Value> {
    Facts { state }.result(intent)
}
struct Facts<'a> {
    state: &'a Value,
}
impl<'a> Facts<'a> {
    fn result(&self, intent: &Intent) -> Option<Value> {
        match &intent.action {
            Action::Planning { change } => super::super::planning::result(self.state, change)
                .ok()
                .flatten(),
            Action::Create { title } => {
                let values = self.list("projects");
                let found = values
                    .iter()
                    .filter(|project| project["title"] == *title && project["public"] == false)
                    .collect::<Vec<_>>();
                (found.len() == 1).then(|| found[0].clone())
            }
            Action::Configure { changes } => {
                let project = &self.state["project"];
                let expected = [
                    ("title", json!(changes.title)),
                    ("shortDescription", json!(changes.description)),
                    ("readme", json!(changes.readme)),
                    ("public", json!(changes.public)),
                ];
                expected
                    .iter()
                    .all(|(key, value)| value.is_null() || project[*key] == *value)
                    .then(|| project.clone())
            }
            Action::Link {
                repository,
                present,
            } => {
                let found = self
                    .list("repositories")
                    .iter()
                    .find(|value| value["id"] == *repository);
                (found.is_some() == *present)
                    .then(|| json!({"repository":repository,"present":present}))
            }
            Action::Candidates { issues, present } => self.candidates(issues, *present),
            Action::Field {
                name,
                datatype,
                options,
            } => {
                let found = self
                    .list("fields")
                    .iter()
                    .filter(|field| field["name"] == *name)
                    .collect::<Vec<_>>();
                let [field] = found.as_slice() else {
                    return None;
                };
                let kind = serde_json::to_value(datatype).expect("native field type");
                (field["dataType"] == kind
                    && field["authority"] == "project"
                    && choices(field, options))
                .then(|| (*field).clone())
            }
            Action::Rename {
                field,
                name,
                options,
            } => {
                let field = self.find("fields", field)?;
                let matches = field["name"] == *name
                    && options
                        .as_ref()
                        .is_none_or(|options| choices(field, options));
                matches.then(|| field.clone())
            }
            Action::Remove { field } => self
                .find("fields", field)
                .is_none()
                .then(|| json!({"field":field,"present":false})),
            Action::Detach { field, origin } => {
                let absent = self.find("fields", field).is_none();
                (absent && self.state["origin"]["id"] == *origin)
                .then(|| json!({"field":field,"present":false,"preserved_origin":self.state["origin"]}))
            }
            Action::View {
                name,
                layout,
                filter,
                visible,
                group,
                vertical,
                sort,
            } => {
                let found = self
                    .list("views")
                    .iter()
                    .filter(|view| view["name"] == *name)
                    .collect::<Vec<_>>();
                let [view] = found.as_slice() else {
                    return None;
                };
                let expected = layout.native();
                let actual = view["sortByFields"]["nodes"].as_array()?.iter().map(|row|json!({"field":row["field"]["id"],"direction":row["direction"].as_str().map(str::to_lowercase)})).collect::<Vec<_>>();
                let actual = json!({"layout":view["layout"],"filter":view["filter"],"visible":references(&view["configuration"]["visibleFields"]),"group":references(&view["groupByFields"]),"vertical":references(&view["verticalGroupByFields"]),"sort":actual});
                let expected = json!({"layout":expected,"filter":filter,"visible":visible,"group":group,"vertical":vertical,"sort":sort});
                (actual == expected).then(|| (*view).clone())
            }
            Action::Revise {
                view,
                name,
                layout,
                filter,
                visible,
            } => {
                let view = self.find("views", view)?;
                let actual = json!({"name":view["name"],"layout":view["layout"],"filter":view["filter"],"visible":references(&view["configuration"]["visibleFields"])});
                let expected = json!({"name":name.as_ref().map(|name|json!(name)).unwrap_or_else(||view["name"].clone()),"layout":layout.as_ref().map(|layout|json!(layout.native())).unwrap_or_else(||view["layout"].clone()),"filter":filter.as_ref().map(|filter|json!(filter)).unwrap_or_else(||view["filter"].clone()),"visible":visible.clone().unwrap_or_else(||references(&view["configuration"]["visibleFields"]))});
                (actual == expected).then(|| view.clone())
            }
            Action::Retire { view } => self
                .find("views", view)
                .is_none()
                .then(|| json!({"view":view,"present":false})),
            Action::Workflow { workflow } => self
                .find("workflows", workflow)
                .is_none()
                .then(|| json!({"workflow":workflow,"present":false})),
            Action::Record { body } => {
                let matches = self
                    .list("statusUpdates")
                    .iter()
                    .filter(|update| update["body"] == *body)
                    .collect::<Vec<_>>();
                (matches.len() == 1).then(|| matches[0].clone())
            }
        }
    }

    fn list(&self, name: &str) -> &'a [Value] {
        self.state[name]
            .as_array()
            .map(Vec::as_slice)
            .unwrap_or_default()
    }

    fn find(&self, name: &str, id: &str) -> Option<&'a Value> {
        self.list(name).iter().find(|value| value["id"] == id)
    }

    fn candidates(&self, issues: &[String], present: bool) -> Option<Value> {
        let mut result = Vec::new();
        for issue in issues {
            let found = self
                .list("items")
                .iter()
                .filter(|item| item["content"]["id"] == *issue)
                .collect::<Vec<_>>();
            if (present && found.len() != 1) || (!present && !found.is_empty()) {
                return None;
            }
            result.push(
            json!({"issue":issue,"item":found.first().map(|item|&item["id"]),"present":present}),
        );
        }
        Some(json!(result))
    }
}

fn choices(field: &Value, requested: &[Choice]) -> bool {
    let key = if field["dataType"] == "MULTI_SELECT" {
        "multiSelectOptions"
    } else {
        "options"
    };
    let Some(actual) = field[key].as_array() else {
        return requested.is_empty();
    };
    actual.len() == requested.len()
        && requested
            .iter()
            .all(|option| actual.iter().any(|actual| choice(actual, option)))
}

fn choice(actual: &Value, option: &Choice) -> bool {
    let identity = option.id.as_ref().is_none_or(|id| actual["id"] == *id);
    let presentation = actual["name"] == option.name
        && actual["color"] == option.color
        && actual["description"] == option.description;
    identity && presentation
}

fn references(connection: &Value) -> Vec<String> {
    connection["nodes"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|field| field["id"].as_str().map(str::to_owned))
        .collect()
}
