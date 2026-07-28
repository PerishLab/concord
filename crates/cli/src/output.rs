use concord_core::{Audit, Error, Plan, Result};
use serde_json::json;

pub fn plan(plan: &Plan, json_output: bool) -> Result<()> {
    if json_output {
        println!(
            "{}",
            serde_json::to_string_pretty(plan)
                .map_err(|error| Error::new(format!("cannot encode plan: {error}")))?
        );
        return Ok(());
    }
    println!("plan: {}", plan.operation);
    for action in &plan.actions {
        println!("  {} {} ({})", action.verb, action.target, action.detail);
    }
    Ok(())
}

pub fn applied(json_output: bool) {
    if json_output {
        return;
    }
    println!("applied");
}

pub fn mutation(plan: &Plan, result: Option<serde_json::Value>, json_output: bool) -> Result<()> {
    if json_output {
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({"plan": plan, "result": result}))
                .map_err(|error| Error::new(format!("cannot encode result: {error}")))?
        );
        return Ok(());
    }
    self::plan(plan, false)?;
    if plan.applied {
        applied(false);
    }
    if let Some(result) = result {
        value(result, false);
    }
    Ok(())
}

pub fn audit(audit: &Audit, json_output: bool) -> Result<()> {
    if json_output {
        println!(
            "{}",
            serde_json::to_string_pretty(audit)
                .map_err(|error| Error::new(format!("cannot encode audit: {error}")))?
        );
    } else {
        println!("audit: {}", audit.target);
        if audit.ok() {
            println!("  true to the protocol");
        }
        for fault in &audit.faults {
            println!("  {}: {}: {}", fault.kind, fault.path, fault.message);
        }
    }
    if audit.ok() {
        Ok(())
    } else {
        Err(Error::new(format!(
            "protocol audit found {} fault(s)",
            audit.faults.len()
        )))
    }
}

pub fn value(value: serde_json::Value, json_output: bool) {
    if json_output {
        println!(
            "{}",
            serde_json::to_string_pretty(&value).expect("JSON value should encode")
        );
        return;
    }
    match value {
        serde_json::Value::Array(values) => {
            for value in values {
                println!("{}", plain(&value));
            }
        }
        value => println!("{}", plain(&value)),
    }
}

fn plain(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(value) => value.clone(),
        value => serde_json::to_string(value).unwrap_or_else(|_| json!(null).to_string()),
    }
}
