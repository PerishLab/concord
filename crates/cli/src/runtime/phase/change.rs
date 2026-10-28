use super::{
    Binding, fault,
    model::{Intent, Plan},
    read,
    wire::Client,
};
use concord_core::Result;
use serde_json::{Value, json};

pub(super) fn qualify(binding: &Binding, intent: &Intent, state: &Value) -> Result<()> {
    if intent.reason.trim().is_empty() || intent.reason.chars().count() > 280 {
        return Err(fault(
            "reason",
            "Phase reason must contain 1..280 characters",
        ));
    }
    if intent
        .value
        .as_ref()
        .is_some_and(|value| !binding.options.contains_key(value))
    {
        return Err(fault(
            "intent",
            "Phase set requires Clarify, Deliver, Verify or Settle; explicit null clears",
        ));
    }
    let issue = &state["issue"];
    if !matches!(
        issue["issueType"]["name"].as_str(),
        Some("Feature" | "Task" | "Bug")
    ) || issue["state"] != "OPEN"
    {
        return Err(fault(
            "custody",
            "Phase writes require an ordinary open Issue; Auto, closed and unknown types are read-only",
        ));
    }
    if issue["viewerCanUpdate"] != true {
        return Err(fault(
            "permission",
            "native provider does not report Issue update capability",
        ));
    }
    Ok(())
}

pub(super) async fn apply(client: &Client<'_>, plan: &Plan) -> Result<Value> {
    let current = read::snapshot(client, &plan.binding, &plan.issue).await?;
    qualify(&plan.binding, &plan.intent, &current)?;
    if current != plan.before {
        return Err(fault(
            "stale",
            "native Phase or Issue facts changed; no write attempted; recover read-only or prepare again",
        ));
    }
    let old = read::value(&plan.binding, &current["issue"])?;
    if old == plan.intent.value {
        return Ok(json!({"outcome":"unchanged","current":current}));
    }
    let input = input(plan);
    client
        .query(MUTATION, json!({"input":input}))
        .await
        .and_then(|data| {
            data["change"]
                .is_object()
                .then_some(data)
                .ok_or_else(|| fault("indeterminate", "native mutation returned no receipt"))
        })
        .map_err(|error| uncertain(&error))?;
    let after = read::snapshot(client, &plan.binding, &plan.issue)
        .await
        .map_err(|error| uncertain(&error))?;
    if read::value(&plan.binding, &after["issue"]).map_err(|error| uncertain(&error))?
        != plan.intent.value
        || protected(&current, &plan.binding) != protected(&after, &plan.binding)
    {
        return Err(fault(
            "indeterminate",
            "independent native readback does not confirm the requested Phase and preserved facts; recover read-only, never replay",
        ));
    }
    Ok(
        json!({"outcome":"confirmed-native-state","before":current,"current":after,"acceptance":{"before_updated":current["issue"]["updatedAt"],"after_updated":after["issue"]["updatedAt"],"semantics":"Phase is not an acceptance judgment; re-evaluate current acceptance separately"},"semantics":"one field mutation and independent readback; no remote CAS, idempotency or transaction guarantee"}),
    )
}

fn input(plan: &Plan) -> Value {
    let mut value = json!({"fieldId":plan.binding.field,"rationale":plan.intent.reason});
    match &plan.intent.value {
        Some(name) => value["singleSelectOptionId"] = json!(plan.binding.options[name]),
        None => value["delete"] = json!(true),
    }
    json!({"issueId":plan.before["issue"]["id"],"issueFields":[value]})
}

fn protected(state: &Value, binding: &Binding) -> Value {
    let mut value = state.clone();
    value["issue"]
        .as_object_mut()
        .expect("native Issue")
        .remove("updatedAt");
    let values = value["issue"]["issueFieldValues"]["nodes"]
        .as_array()
        .expect("native fields");
    let mut other = values
        .iter()
        .filter(|value| value["field"]["id"] != binding.field)
        .cloned()
        .collect::<Vec<_>>();
    other.sort_by_key(|value| value["id"].to_string());
    value["issue"]["issueFieldValues"] = json!(other);
    value
}

fn uncertain(error: &concord_core::Error) -> concord_core::Error {
    fault(
        "indeterminate",
        format!(
            "Phase write may have completed; recover read-only without replay: {}",
            error.message()
        ),
    )
}

const MUTATION: &str = "mutation($input:SetIssueFieldValueInput!){change:setIssueFieldValue(input:$input){clientMutationId}}";
