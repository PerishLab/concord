use super::super::fault;
use super::{
    model::{Action, Changes, Data, Intent},
    qualify,
};
use concord_core::Result;
use serde_json::{Value, json};

pub(in crate::project) struct Step {
    pub mutation: &'static str,
    pub input: Value,
    pub check: Intent,
}

pub(super) fn steps(state: &Value, intent: &Intent) -> Result<Vec<Step>> {
    Builder { state }.steps(intent)
}

struct Builder<'a> {
    state: &'a Value,
}

impl Builder<'_> {
    fn steps(&self, intent: &Intent) -> Result<Vec<Step>> {
        qualify::intent(self.state, intent)?;
        let project = &self.state["project"]["id"];
        let (mutation, input) = match &intent.action {
            Action::Planning { .. } => return super::super::planning::build(self.state, intent),
            Action::Create { title } => (
                "createProjectV2",
                json!({"ownerId":self.state["owner"]["id"],"title":title}),
            ),
            Action::Configure { changes } => ("updateProjectV2", self.configure(changes)),
            Action::Link {
                repository,
                present,
            } => (
                if *present {
                    "linkProjectV2ToRepository"
                } else {
                    "unlinkProjectV2FromRepository"
                },
                json!({"projectId":project,"repositoryId":repository}),
            ),
            Action::Candidates { .. } => return self.candidates(intent),
            Action::Field { .. } => ("createProjectV2Field", self.field(&intent.action)),
            Action::Rename { .. } => ("updateProjectV2Field", self.rename(&intent.action)?),
            Action::Remove { field } | Action::Detach { field, .. } => {
                ("deleteProjectV2Field", json!({"fieldId":field}))
            }
            Action::View { .. } => ("rest:view", self.view(&intent.action)?),
            Action::Revise { .. } => ("updateProjectV2View", self.revise(&intent.action)),
            Action::Retire { view } => ("deleteProjectV2View", json!({"viewId":view})),
            Action::Workflow { workflow } => {
                ("deleteProjectV2Workflow", json!({"workflowId":workflow}))
            }
            Action::Record { body } => (
                "createProjectV2StatusUpdate",
                json!({"projectId":project,"body":body}),
            ),
        };
        Ok(vec![Step {
            mutation,
            input,
            check: intent.clone(),
        }])
    }

    fn configure(&self, changes: &Changes) -> Value {
        let mut input = json!({"projectId":self.state["project"]["id"]});
        for (key, value) in [
            ("title", json!(changes.title)),
            ("shortDescription", json!(changes.description)),
            ("readme", json!(changes.readme)),
            ("public", json!(changes.public)),
        ] {
            if !value.is_null() {
                input[key] = value;
            }
        }
        input
    }

    fn field(&self, action: &Action) -> Value {
        let Action::Field {
            name,
            datatype,
            options,
        } = action
        else {
            unreachable!("field action");
        };
        let mut input =
            json!({"projectId":self.state["project"]["id"],"name":name,"dataType":datatype});
        match datatype {
            Data::Single => input["singleSelectOptions"] = json!(options),
            Data::Multiple => input["multiSelectOptions"] = json!(options),
            _ => {}
        }
        input
    }

    fn rename(&self, action: &Action) -> Result<Value> {
        let Action::Rename {
            field,
            name,
            options,
        } = action
        else {
            unreachable!("field update");
        };
        let mut input = json!({"fieldId":field,"name":name});
        if let Some(options) = options {
            let field = qualify::find(self.state, "fields", field)?;
            let key = if field["dataType"] == "SINGLE_SELECT" {
                "singleSelectOptions"
            } else {
                "multiSelectOptions"
            };
            input[key] = json!(options);
        }
        Ok(input)
    }

    fn revise(&self, action: &Action) -> Value {
        let Action::Revise {
            view,
            name,
            layout,
            filter,
            visible,
        } = action
        else {
            unreachable!("view update");
        };
        let mut input = json!({"viewId":view});
        if let Some(layout) = layout {
            input["layout"] = json!(layout.native());
        }
        for (key, value) in [("name", json!(name)), ("filter", json!(filter))] {
            if !value.is_null() {
                input[key] = value;
            }
        }
        if let Some(visible) = visible {
            input["configuration"] = json!({"visibleFieldIds":visible});
        }
        input
    }

    fn candidates(&self, intent: &Intent) -> Result<Vec<Step>> {
        let Action::Candidates { issues, present } = &intent.action else {
            unreachable!("candidate change");
        };
        let mut steps = Vec::new();
        for issue in issues {
            let item = self.state["items"]
                .as_array()
                .expect("qualified items")
                .iter()
                .find(|item| item["content"]["id"] == *issue);
            let (mutation, input) = match (*present, item) {
                (true, None) => (
                    "addProjectV2ItemById",
                    json!({"projectId":self.state["project"]["id"],"contentId":issue}),
                ),
                (false, Some(item)) => (
                    "deleteProjectV2Item",
                    json!({"projectId":self.state["project"]["id"],"itemId":item["id"]}),
                ),
                _ => continue,
            };
            let check = Intent {
                reason: intent.reason.clone(),
                action: Action::Candidates {
                    issues: vec![issue.clone()],
                    present: *present,
                },
            };
            steps.push(Step {
                mutation,
                input,
                check,
            });
        }
        Ok(steps)
    }

    fn view(&self, action: &Action) -> Result<Value> {
        let Action::View {
            name,
            layout,
            filter,
            visible,
            group,
            vertical,
            sort,
        } = action
        else {
            unreachable!("view action");
        };
        let visible = self.ids(visible)?;
        let group = self.ids(group)?;
        let vertical = self.ids(vertical)?;
        let sort = sort
            .iter()
            .map(|sort| Ok(json!([self.column(&sort.field)?, sort.direction])))
            .collect::<Result<Vec<Value>>>()?;
        Ok(
            json!({"name":name,"layout":layout,"filter":filter,"visible_fields":visible,"group_by":group,"vertical_group_by":vertical,"sort_by":sort}),
        )
    }

    fn ids(&self, fields: &[String]) -> Result<Vec<u64>> {
        fields.iter().map(|field| self.column(field)).collect()
    }

    fn column(&self, id: &str) -> Result<u64> {
        if !self.state["fields"]
            .as_array()
            .expect("native fields")
            .iter()
            .any(|field| field["id"] == id)
        {
            return Err(fault(
                "unsupported",
                "the provider cannot independently project this selected view field; no substitute column or unverified write is created",
            ));
        }
        self.state["columns"]
            .as_array()
            .expect("qualified columns")
            .iter()
            .find(|field| field["node_id"] == id)
            .and_then(|field| field["id"].as_u64())
            .ok_or_else(|| {
                fault(
                    "field",
                    "view references an unknown native REST field identity",
                )
            })
    }
}
