mod choice;
mod read;
mod scope;
#[cfg(test)]
mod tests;

use super::{fault, wire::Client};
use crate::args::project::Observe;
use concord_core::Result;
use serde_json::{Value, json};

pub(super) async fn run(
    args: Observe,
    output: bool,
    phase: Option<&crate::phase::Binding>,
) -> Result<()> {
    let client = Client {
        command: &args.command,
        timeout: args.timeout,
    };
    let started = super::now();
    let state = read::state(&client, &args).await?;
    let day = i64::try_from((started + 8 * 3600) / 86400).map_err(|_| {
        fault(
            "date",
            "current calendar day is outside the supported range",
        )
    })?;
    let mut result = select(&state, day)?;
    if !result["issue"].is_null() {
        read::confirm(&client, &result["issue"]).await?;
        let issue = &result["issue"];
        let target = format!(
            "{}#{}",
            issue["repository"]["nameWithOwner"]
                .as_str()
                .unwrap_or_default(),
            issue["number"]
        );
        result["phase"] = crate::phase::observe(
            phase,
            &target,
            issue["id"].as_str().expect("native Issue"),
            (&args.command, args.timeout),
        )
        .await;
        read::confirm(&client, &result["issue"]).await?;
    }
    read::finish(&client, &state["project"]).await?;
    let finished = super::now();
    if (started + 8 * 3600) / 86400 != (finished + 8 * 3600) / 86400 {
        return Err(fault(
            "changed",
            "local calendar day changed during selection; read again",
        ));
    }
    result["schema"] = json!("concord.project-next/v1");
    result["project"] = json!({"id":state["project"]["id"],"url":state["project"]["url"],"title":state["project"]["title"],"updatedAt":state["project"]["updatedAt"]});
    result["started_at"] = json!(started);
    result["observed_at"] = json!(finished);
    result["timezone"] = json!("Asia/Shanghai");
    result["semantics"] = json!(
        "live planning observation, not execution readiness, ownership or permission; local Members/Claims/activity are not observed; use Issue preflight before acting"
    );
    if output {
        crate::output::value(json!({"next":result,"version":1}), true);
    } else if let Some(url) = result["issue"]["url"].as_str() {
        println!(
            "{url}\n{}\n{}",
            result["issue"]["title"].as_str().unwrap_or(""),
            result["reason"].as_str().unwrap_or("")
        );
        println!("Phase: {}", result["phase"]);
    }
    Ok(())
}

fn select(state: &Value, day: i64) -> Result<Value> {
    if state["project"]["closed"] == true {
        return Ok(empty("Project is closed"));
    }
    let Some(scope) = scope::Scope::find(&state["fields"], day)? else {
        return Ok(empty(
            "planning fields or current cycle are missing or ambiguous",
        ));
    };
    let item = choice::select(&state["items"], &scope, &state["project"])?;
    let mut result = json!({"issue":null,"item":null,"priority":null,"cycle":scope.period,"fields":{"cycle":scope.cycle,"priority":scope.priority},"reason":"no eligible ordinary open Issue in the current cycle"});
    if let Some(item) = item {
        result["item"] = item["id"].clone();
        result["issue"] = item["content"].clone();
        result["priority"] = scope.priority(item)?;
        result["reason"] = json!(
            "first eligible ordinary open Issue in current-cycle native item order; priority is reported without reranking"
        );
    }
    Ok(result)
}

fn empty(reason: &str) -> Value {
    json!({"issue":null,"item":null,"priority":null,"cycle":null,"fields":null,"reason":reason})
}
