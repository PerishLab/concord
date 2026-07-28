use concord_core::{Audit, Error, Plan, Result};
use plumb::skill::{Done, Record};
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

pub fn skill_done(action: &str, done: &Done, json_output: bool) -> Result<()> {
    if json_output {
        value(
            json!({
                "action": action,
                "changed": done.kept.iter().map(|seat| json!({
                    "agent": seat.agent,
                    "path": seat.path.display().to_string(),
                })).collect::<Vec<_>>(),
                "skipped": done.left.iter().map(|skip| json!({
                    "path": skip.path.display().to_string(),
                    "reason": skip.note,
                })).collect::<Vec<_>>(),
            }),
            true,
        );
        return Ok(());
    }
    for seat in &done.kept {
        println!("{action} {} {}", seat.agent, seat.path.display());
    }
    for skip in &done.left {
        println!("skipped {}: {}", skip.path.display(), skip.note);
    }
    Ok(())
}

pub fn skill_records(records: &[Record], json_output: bool) -> Result<()> {
    if json_output {
        value(
            serde_json::Value::Array(
                records
                    .iter()
                    .map(|record| {
                        json!({
                            "agent": record.agent,
                            "path": record.path.display().to_string(),
                            "version": record.version,
                            "url": record.url,
                            "sha256": record.sha,
                        })
                    })
                    .collect(),
            ),
            true,
        );
        return Ok(());
    }
    if records.is_empty() {
        println!("no managed Concord skill");
    }
    for record in records {
        println!(
            "{} {} {}",
            record.agent,
            record.version,
            record.path.display()
        );
    }
    Ok(())
}

fn plain(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(value) => value.clone(),
        value => serde_json::to_string(value).unwrap_or_else(|_| json!(null).to_string()),
    }
}
