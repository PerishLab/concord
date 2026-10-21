use super::super::{fault, wire::Client};
use super::{
    build::{self, Step},
    desired,
    journal::{self, Journal, Phase},
    model::{Action, Intent, Plan},
    native, qualify, read,
};
use concord_core::{Error, Result};
use serde_json::{Value, json};

pub(super) async fn run(client: &Client<'_>, plan: &Plan, pages: u16) -> Result<Value> {
    Runner { client, pages }.run(plan).await
}
pub(super) async fn recover(client: &Client<'_>, plan: &Plan, pages: u16) -> Result<Value> {
    Runner { client, pages }.recover(plan).await
}
struct Runner<'a, 'b> {
    client: &'a Client<'b>,
    pages: u16,
}

impl Runner<'_, '_> {
    async fn run(&self, plan: &Plan) -> Result<Value> {
        let before = read::state(self.client, &plan.target, &plan.intent, self.pages).await?;
        if before != plan.before {
            return Err(fault(
                "stale",
                "native facts changed; recover read-only or prepare from current facts; no write was attempted",
            ));
        }
        let steps = build::steps(&before, &plan.intent)?;
        let journal = Journal {
            client: self.client,
            pages: self.pages,
        };
        let mut completed = Vec::new();
        let mut state = before.clone();
        let ordinary = !matches!(
            plan.intent.action,
            Action::Create { .. } | Action::Record { .. }
        );
        if ordinary && !steps.is_empty() {
            let requested=journal.publish(plan,&state,Phase::Requested).await.map_err(|error|Error::detailed("concord.project.record.indeterminate","requested planning-record outcome is unknown; action steps were not attempted",json!({"stage":"requested-record","action_steps":"not-attempted","completed":completed,"cause":error.message()})))?;
            state = read::state(self.client, &plan.target, &plan.intent, self.pages)
                .await
                .map_err(|error| interrupted(error, &completed, 0))?;
            let mut original = state.clone();
            original["statusUpdates"]
                .as_array_mut()
                .expect("observed planning records")
                .retain(|update| *update != requested["update"]);
            if comparable(&before) != comparable(&original) {
                return Err(interrupted(
                    fault(
                        "stale",
                        "native facts changed while recording the requested change",
                    ),
                    &completed,
                    0,
                ));
            }
        }
        let context = state.clone();
        for (index, step) in steps.iter().enumerate() {
            qualify::intent(&state, &step.check)
                .map_err(|error| interrupted(error, &completed, index))?;
            fresh(&before, &state, &step.check)
                .map_err(|error| interrupted(error, &completed, index))?;
            let receipt = native::perform(self.client, &state, step)
                .await
                .map_err(|error| uncertain(error, &completed, index))?;
            state = self
                .confirm(plan, step, &context)
                .await
                .map_err(|error| uncertain(error, &completed, index))?;
            let result = desired::result(&state, &step.check).ok_or_else(|| {
                uncertain(
                    fault(
                        "readback",
                        "native current facts do not confirm the requested result",
                    ),
                    &completed,
                    index,
                )
            })?;
            completed.push(json!({"step":index,"native_result":result,"provider_receipt":receipt}));
            if ordinary && contextchanged(&context, &state, &step.check, false) {
                return Err(Error::detailed(
                    "concord.project.context.changed",
                    "native planning context changed while the change was in flight; confirmed results are retained and further writes were not attempted",
                    json!({"completed":completed,"current_project":state["project"]}),
                ));
            }
        }
        let result = desired::result(&state, &plan.intent).ok_or_else(|| {
            uncertain(
                fault("readback", "native batch result remains incomplete"),
                &completed,
                steps.len(),
            )
        })?;
        if matches!(plan.intent.action, Action::Create { .. }) {
            state = self
                .created(plan, &result)
                .await
                .map_err(|error| uncertain(error, &completed, steps.len()))?;
        }
        let record = if steps.is_empty() || matches!(plan.intent.action, Action::Record { .. }) {
            Value::Null
        } else {
            journal.publish(plan,&state,Phase::Confirmed).await.map_err(|error|Error::detailed("concord.project.record.indeterminate","native action state is confirmed, but its planning-record outcome is unknown; recover without replaying the action",json!({"action":"confirmed","completed":completed,"native_result":result,"cause":error.message()})))?
        };
        let identity = record.get("project").unwrap_or(&state["project"]);
        Ok(
            json!({"schema":"concord.project-result/v1","outcome":if steps.is_empty(){"unchanged"}else{"confirmed-native-state"},"project":identity,"automatic_behavior":{"native":state["workflows"],"external":"unknown"},"completed":completed,"result":result,"record":record,"coverage":"requested properties; native view Type visibility and external automation remain provider projection gaps","semantics":"independent native readback; no provider transaction, execution delegation, or Issue lifecycle mutation request"}),
        )
    }

    async fn confirm(&self, plan: &Plan, step: &Step, context: &Value) -> Result<Value> {
        let ordinary = !matches!(
            plan.intent.action,
            Action::Create { .. } | Action::Record { .. }
        );
        for attempt in 0..3 {
            let state = read::state(self.client, &plan.target, &plan.intent, self.pages).await?;
            if desired::result(&state, &step.check).is_some() {
                return Ok(state);
            }
            if ordinary && contextchanged(context, &state, &step.check, true) {
                return Err(fault(
                    "context.changed",
                    "planning context changed while native confirmation was pending; no further write was attempted",
                ));
            }
            if attempt < 2 {
                tokio::time::sleep(std::time::Duration::from_millis(250)).await;
            }
        }
        Err(fault(
            "readback",
            "three fresh native observations did not confirm the requested result",
        ))
    }

    async fn recover(&self, plan: &Plan) -> Result<Value> {
        let mut state = read::state(self.client, &plan.target, &plan.intent, self.pages).await?;
        let result = desired::result(&state, &plan.intent);
        if let (Action::Create { .. }, Some(project)) = (&plan.intent.action, &result) {
            state = self.created(plan, project).await?;
        }
        let records = journal::records(&state, plan);
        Ok(
            json!({"schema":"concord.project-recovery/v1","desired_state":if result.is_some(){"observed"}else{"unconfirmed"},"native_result":result,"planning_records":records,"current_project":state.get("project"),"current_owner":state.get("owner"),"semantics":"read-only recovery; current facts do not prove which actor performed an earlier uncertain write; no write was repeated; inspect partial membership and prepare a new narrow intent from current facts","current":state}),
        )
    }

    async fn created(&self, plan: &Plan, project: &Value) -> Result<Value> {
        let target = format!("{}/{}", plan.target, project["number"]);
        let intent = Intent {
            reason: plan.intent.reason.clone(),
            action: Action::Record {
                body: plan.intent.reason.clone(),
            },
        };
        let state = read::state(self.client, &target, &intent, self.pages).await?;
        if state["project"]["id"] != project["id"] || state["project"]["public"] != false {
            return Err(fault(
                "identity",
                "created Project identity/private visibility is unconfirmed",
            ));
        }
        Ok(state)
    }
}

fn comparable(state: &Value) -> Value {
    let mut state = state.clone();

    if let Some(project) = state["project"].as_object_mut() {
        project.remove("updatedAt");
    }
    state
}

fn fresh(before: &Value, current: &Value, intent: &Intent) -> Result<()> {
    let Action::Candidates { issues, .. } = &intent.action else {
        return Ok(());
    };
    for id in issues {
        let previous = before["issues"]
            .as_array()
            .expect("qualified Issues")
            .iter()
            .find(|issue| issue["id"] == *id);
        let actual = current["issues"]
            .as_array()
            .expect("observed Issues")
            .iter()
            .find(|issue| issue["id"] == *id);
        let previousitem = before["items"]
            .as_array()
            .expect("qualified items")
            .iter()
            .find(|item| item["content"]["id"] == *id);
        let actualitem = current["items"]
            .as_array()
            .expect("observed items")
            .iter()
            .find(|item| item["content"]["id"] == *id);
        if previousitem != actualitem {
            return Err(fault(
                "stale",
                "candidate membership changed before its own mutation",
            ));
        }
        if previous != actual {
            return Err(fault(
                "stale",
                "candidate Issue facts changed before its own mutation",
            ));
        }
    }
    Ok(())
}

fn uncertain(error: Error, completed: &[Value], step: usize) -> Error {
    Error::detailed(
        "concord.project.indeterminate",
        "native change/readback failed; preceding confirmations are retained; recover current facts before any further write",
        json!({"outcome":"unknown","completed":completed,"unconfirmed_step":step,"later_steps":"not-attempted","cause":{"code":error.code(),"message":error.message()}}),
    )
}

fn contextchanged(before: &Value, current: &Value, intent: &Intent, pending: bool) -> bool {
    let mut expected = before["project"].clone();
    let mut actual = current["project"].clone();
    expected
        .as_object_mut()
        .expect("native Project")
        .remove("updatedAt");
    actual
        .as_object_mut()
        .expect("native Project")
        .remove("updatedAt");
    if let Action::Configure { changes } = &intent.action {
        for (key, value) in [
            ("title", json!(changes.title)),
            ("shortDescription", json!(changes.description)),
            ("readme", json!(changes.readme)),
            ("public", json!(changes.public)),
        ] {
            if value.is_null() {
                continue;
            }
            if pending && actual[key] == expected[key] {
                actual[key] = value.clone();
            }
            expected[key] = value;
        }
    }
    expected != actual || before["statusUpdates"] != current["statusUpdates"]
}

fn interrupted(error: Error, completed: &[Value], step: usize) -> Error {
    Error::detailed(
        "concord.project.interrupted",
        "a current precondition refused the next action; preceding confirmations are retained and this action was not attempted",
        json!({"outcome":"not-attempted","completed":completed,"next_step":step,"later_steps":"not-attempted","cause":{"code":error.code(),"message":error.message()}}),
    )
}
