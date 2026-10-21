use super::super::{fault, wire::Client};
use super::{
    build, desired,
    model::{Action, Intent, Plan},
    native, read,
};
use concord_core::Result;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

pub(super) enum Phase {
    Requested,
    Confirmed,
}

pub(super) struct Journal<'a, 'b> {
    pub client: &'a Client<'b>,
    pub pages: u16,
}

impl Journal<'_, '_> {
    pub async fn publish(&self, plan: &Plan, state: &Value, phase: Phase) -> Result<Value> {
        let status = match phase {
            Phase::Requested => "requested",
            Phase::Confirmed => "confirmed-native-state",
        };
        let kind = serde_json::to_value(&plan.intent.action).expect("typed Project action")["kind"]
            .as_str()
            .expect("action tag")
            .to_owned();
        let body = format!(
            "{}\n\nAction: {kind}\nStatus: {status}\nProject: {}\n\n{} {status} -->",
            plan.intent.reason,
            state["project"]["url"]
                .as_str()
                .expect("qualified Project URL"),
            marker(plan)
        );
        let intent = Intent {
            reason: plan.intent.reason.clone(),
            action: Action::Record { body },
        };
        let steps = build::steps(state, &intent)?;
        let receipt = native::perform(self.client, state, &steps[0]).await?;
        let target = state["project"]["url"]
            .as_str()
            .expect("qualified Project URL");
        let current = read::state(self.client, target, &intent, self.pages).await?;
        let record = desired::result(&current, &intent)
            .ok_or_else(|| fault("readback", "planning update is not independently confirmed"))?;
        Ok(json!({"update":record,"project":current["project"],"provider_receipt":receipt}))
    }
}

pub(super) fn records(state: &Value, plan: &Plan) -> Vec<Value> {
    let marker = marker(plan);
    state["statusUpdates"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|update| {
            update["body"]
                .as_str()
                .is_some_and(|body| body.contains(&marker))
        })
        .cloned()
        .collect()
}

fn marker(plan: &Plan) -> String {
    let bytes = serde_json::to_vec(plan).expect("native operation plan");
    format!("<!-- concord.project-change/v1 {:x}", Sha256::digest(bytes))
}
