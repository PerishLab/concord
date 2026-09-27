mod activity;
mod member;
mod task;

use concord_core::occupancy::Occupancy;
use concord_core::{ClaimOverlap, Error, Result};
use plumb::skill::{Done, Record, Report};
use serde_json::json;

pub use activity::{activity, issue_activity, unavailable};
pub use member::{issue_status as issue_member_status, status as member_status};
pub use task::brief as task_brief;

pub fn occupancy(occupancy: &Occupancy, output: bool) {
    if occupancy.conflicts.is_empty() {
        return;
    }
    let warning = json!({
        "warning": {
            "code": "concord.occupancy.concurrent_session",
            "message": "another session has current non-exclusive occupancy on an intersecting managed write subject",
            "lease_seconds": occupancy.lease,
            "current": occupancy.current,
            "conflicts": occupancy.conflicts,
            "semantics": "a heartbeat proves only a Concord write; expiry does not prove session death",
        }
    });
    if output {
        eprintln!(
            "{}",
            serde_json::to_string(&warning).expect("occupancy warning JSON should encode")
        );
        return;
    }
    eprintln!(
        "concord: warning: another session has non-exclusive occupancy on this write surface"
    );
    for conflict in &occupancy.conflicts {
        eprintln!(
            "  {} {} wrote {} at {} on {:?}",
            conflict.holder.agent.name(),
            conflict.holder.session,
            conflict.holder.operation,
            conflict.holder.heartbeat,
            conflict.subjects
        );
    }
}

pub fn blind(error: &Error, output: bool) {
    let warning = json!({
        "warning": {
            "code": "concord.occupancy.unavailable",
            "message": "session occupancy is unavailable; the primary command remains unaffected",
            "details": {"code": error.code(), "message": error.message()},
        }
    });
    if output {
        eprintln!(
            "{}",
            serde_json::to_string(&warning).expect("occupancy warning JSON should encode")
        );
        return;
    }
    eprintln!(
        "concord: warning: session occupancy is unavailable: {}",
        error.message()
    );
}

pub fn observations(observations: &[ClaimOverlap]) {
    for observation in observations {
        eprintln!(
            "concord: observation: write Claim overlaps {} at {}",
            observation.peer,
            observation.paths.join(", ")
        );
    }
}

pub fn value(value: serde_json::Value, output: bool) {
    if output {
        println!(
            "{}",
            serde_json::to_string(&value).expect("JSON value should encode")
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
