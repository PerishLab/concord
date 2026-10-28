use super::super::fault;
use super::super::field::{limit, text};
use super::model::{Action, Data, Intent};
use concord_core::Result;
use serde_json::Value;
use std::collections::BTreeSet;

pub(super) fn intent(state: &Value, intent: &Intent) -> Result<()> {
    Context { state }.intent(intent)
}
pub(super) fn find<'a>(state: &'a Value, name: &str, id: &str) -> Result<&'a Value> {
    Context { state }.find(name, id)
}

struct Context<'a> {
    state: &'a Value,
}

impl<'a> Context<'a> {
    fn intent(&self, intent: &Intent) -> Result<()> {
        text(&intent.reason, 4000)?;
        if !matches!(intent.action, Action::Create { .. })
            && self.state["project"]["viewerCanUpdate"] != true
        {
            return Err(fault(
                "permission",
                "provider does not report Project update capability; this is not semantic delegation",
            ));
        }
        match &intent.action {
            Action::Planning { change } => super::super::planning::qualify(self.state, change)?,
            Action::Create { title } => {
                text(title, 255)?;
                self.unique("projects", title, None)?;
            }
            Action::Configure { changes } => {
                if let Some(title) = &changes.title {
                    text(title, 255)?;
                }
                if let Some(description) = &changes.description {
                    limit(description, 1024)?;
                }
                if let Some(readme) = &changes.readme {
                    limit(readme, 65000)?;
                }
                let content = changes.title.is_none()
                    && changes.description.is_none()
                    && changes.readme.is_none();
                if content && changes.public.is_none() {
                    return Err(fault("intent", "configuration change is empty"));
                }
            }
            Action::Link { repository, .. } => {
                let actual = &self.state["repository"];
                if actual["__typename"] != "Repository"
                    || actual["id"] != *repository
                    || actual["owner"]["id"] != self.state["project"]["owner"]["id"]
                {
                    return Err(fault(
                        "identity",
                        "repository is missing, foreign or outside the organization Project scope",
                    ));
                }
            }
            Action::Candidates { issues, present } => self.candidates(issues, *present)?,
            Action::Field {
                name,
                datatype,
                options,
            } => {
                text(name, 255)?;
                self.unique("fields", name, None)?;
                super::super::field::choices(options)?;
                if options.iter().any(|option| option.id.is_some()) {
                    return Err(fault("intent", "new options cannot claim existing IDs"));
                }
                if !matches!(datatype, Data::Single | Data::Multiple) && !options.is_empty() {
                    return Err(fault("type", "scalar field does not accept select options"));
                }
            }
            Action::Rename {
                field,
                name,
                options,
            } => {
                let current = super::super::field::owned(self.state, field)?;
                text(name, 255)?;
                self.unique("fields", name, Some(field))?;
                if let Some(options) = options {
                    super::super::field::preserve(current, options)?;
                }
            }
            Action::Remove { field } => super::super::field::empty(self.state, field)?,
            Action::Attach { .. } => {}
            Action::Detach { field, origin } => {
                let field = self.find("fields", field)?;
                if field["authority"] != "issue" || field["issueField"]["id"] != *origin {
                    return Err(fault(
                        "ownership",
                        "detachment must bind an attached Project field and its exact Issue-field origin",
                    ));
                }
            }
            Action::View {
                name,
                visible,
                group,
                vertical,
                sort,
                ..
            } => {
                text(name, 255)?;
                self.unique("views", name, None)?;
                if visible.is_empty() || group.len() > 1 || vertical.len() > 1 {
                    return Err(fault(
                        "view",
                        "view requires explicit visible fields and at most one field per grouping axis",
                    ));
                }
                if sort.len() > 20 {
                    return Err(fault(
                        "bounds",
                        "view sort exceeds twenty explicit criteria",
                    ));
                }
            }
            Action::Revise {
                view,
                name,
                visible,
                ..
            } => {
                self.find("views", view)?;
                if let Some(name) = name {
                    text(name, 255)?;
                    self.unique("views", name, Some(view))?;
                }
                if let Some(visible) = visible {
                    self.visible(visible)?;
                }
            }
            Action::Retire { view } => {
                self.find("views", view)?;
            }
            Action::Workflow { workflow } => {
                self.find("workflows", workflow)?;
            }
            Action::Record { body } => {
                text(body, 60000)?;
                if self.state["statusUpdates"]
                    .as_array()
                    .expect("native updates")
                    .iter()
                    .any(|update| update["body"] == *body)
                {
                    return Err(fault(
                        "ambiguous",
                        "identical planning update already exists; recover its identity instead of blindly appending",
                    ));
                }
            }
        }
        Ok(())
    }

    fn find(&self, name: &str, id: &str) -> Result<&'a Value> {
        self.state[name]
            .as_array()
            .and_then(|values| values.iter().find(|value| value["id"] == id))
            .ok_or_else(|| {
                fault(
                    "identity",
                    format!("{name} does not contain the explicitly selected identity"),
                )
            })
    }

    fn candidates(&self, ids: &[String], present: bool) -> Result<()> {
        if ids.is_empty() || ids.len() > 500 {
            return Err(fault(
                "bounds",
                "candidate change requires 1..500 stable Issue identities",
            ));
        }
        for item in self.state["items"]
            .as_array()
            .expect("observed Project items")
        {
            if item["content"].is_null() {
                return Err(fault(
                    "incomplete",
                    "candidate membership contains an unreadable original object",
                ));
            }
            if ids.iter().any(|id| item["content"]["id"] == *id) && item["isArchived"] == true {
                return Err(fault(
                    "archived",
                    "candidate is archived; preserve its identity and history until an explicit unarchive decision",
                ));
            }
        }
        let mut seen = BTreeSet::new();
        let actual = self.state["issues"]
            .as_array()
            .expect("observed Issue facts");
        for id in ids {
            let issue = actual
                .iter()
                .find(|issue| issue["id"] == *id)
                .ok_or_else(|| fault("identity", "candidate native identity is missing"))?;
            if !seen.insert(id) {
                return Err(fault("intent", "candidate Issue identity repeated"));
            }
            if issue["__typename"] != "Issue"
                || issue["id"] != *id
                || issue["repository"]["owner"]["id"] != self.state["project"]["owner"]["id"]
            {
                return Err(fault(
                    "identity",
                    "candidate is missing, foreign or outside the entrusted organization",
                ));
            }
            if !matches!(
                issue["issueType"]["name"].as_str(),
                Some("Feature" | "Bug" | "Task")
            ) {
                return Err(fault(
                    "custody",
                    "Auto and unknown Issue types are read-only observations",
                ));
            }
            if present && issue["state"] != "OPEN" {
                return Err(fault(
                    "lifecycle",
                    "new candidate must be an original open ordinary Issue",
                ));
            }
        }
        if self.state["workflows"]
            .as_array()
            .expect("observed workflows")
            .iter()
            .any(|workflow| workflow["enabled"] != false)
        {
            return Err(fault(
                "automation",
                "candidate maintenance requires explicitly qualified disabled native workflows; external automation remains unknown",
            ));
        }
        Ok(())
    }

    fn unique(&self, collection: &str, name: &str, except: Option<&String>) -> Result<()> {
        let values = self.state[collection]
            .as_array()
            .expect("observed native collection");
        let key = if collection == "projects" {
            "title"
        } else {
            "name"
        };
        if values
            .iter()
            .any(|value| value[key] == name && except.is_none_or(|id| value["id"] != *id))
        {
            return Err(fault(
                "ambiguous",
                "native name already exists; select its exact identity instead of creating a duplicate",
            ));
        }
        Ok(())
    }

    fn visible(&self, ids: &[String]) -> Result<()> {
        if ids.is_empty() {
            return Err(fault(
                "view",
                "visible field configuration cannot be implicit",
            ));
        }
        for id in ids {
            if !self.state["fields"]
                .as_array()
                .expect("observed native fields")
                .iter()
                .any(|field| field["id"] == *id)
            {
                return Err(fault(
                    "unsupported",
                    "selected visible field cannot be independently projected by the native API",
                ));
            }
        }
        Ok(())
    }
}
