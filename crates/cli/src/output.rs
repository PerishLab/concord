mod member;
mod task;

use concord_core::activity::Activity;
use concord_core::{Error, Result};
use plumb::skill::{Done, Record, Report};
use serde_json::json;

pub use member::status as member_status;
pub use task::brief as task_brief;

pub fn activity(activity: &Activity, output: bool) {
    if activity.recent.is_empty() {
        return;
    }
    let warning = json!({
        "warning": {
            "code": "concord.activity.concurrent_session",
            "message": "another session touched this Task within the recent window; take care",
            "task": activity.task,
            "window_seconds": activity.window,
            "current": activity.current,
            "sessions": activity.recent,
        }
    });
    if output {
        eprintln!(
            "{}",
            serde_json::to_string(&warning).expect("activity warning JSON should encode")
        );
        return;
    }
    eprintln!(
        "concord: warning: {} has recent activity from another session; take care",
        activity.task
    );
    for touch in &activity.recent {
        eprintln!(
            "  {} {} {} at {}",
            touch.agent.name(),
            touch.session,
            touch.operation,
            touch.time
        );
    }
}

pub fn unavailable(task: &str, error: &Error, output: bool) {
    let warning = json!({
        "warning": {
            "code": "concord.activity.unavailable",
            "message": "operator activity is unavailable; the primary command remains unaffected",
            "task": task,
            "details": {
                "code": error.code(),
                "message": error.message(),
            },
        }
    });
    if output {
        eprintln!(
            "{}",
            serde_json::to_string(&warning).expect("activity warning JSON should encode")
        );
        return;
    }
    eprintln!(
        "concord: warning: operator activity is unavailable for {task}: {}",
        error.message()
    );
}

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
