use concord_core::Result;
use plumb::skill::{Done, Record, Report};
use serde_json::json;

pub fn value(value: serde_json::Value, output: bool) {
    if output {
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

pub fn done(action: &str, done: &Done, output: bool) -> Result<()> {
    if output {
        value(
            json!({
                "action": action,
                "changed": done.kept.iter().map(|seat| json!({
                    "agent": seat.agent,
                    "path": seat.path.display().to_string(),
                })).collect::<Vec<_>>(),
                "unchanged": done.same.iter().map(|seat| json!({
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
    for seat in &done.same {
        println!("unchanged {} {}", seat.agent, seat.path.display());
    }
    for skip in &done.left {
        println!("skipped {}: {}", skip.path.display(), skip.note);
    }
    Ok(())
}

pub fn report(operation: &str, report: &Report, output: bool) -> Result<()> {
    if output {
        value(
            json!({
                "operation": operation,
                "channel": report.channel,
                "explicit": report.explicit,
                "target": report.target,
                "seats": report.seats,
            }),
            true,
        );
        return Ok(());
    }
    println!("target {} {}", report.channel, report.target.version);
    if report.seats.is_empty() {
        println!("unmanaged");
    }
    for status in &report.seats {
        println!(
            "{} {} -> {} {} {} {}",
            status.agent,
            status.installed,
            report.target.version,
            status.state,
            status.action,
            status.path.display()
        );
    }
    Ok(())
}

pub fn records(records: &[Record], output: bool) -> Result<()> {
    if output {
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
