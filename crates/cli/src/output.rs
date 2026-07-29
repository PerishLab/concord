mod audit_report;

use concord_core::{Audit, Error, LandingProof, Plan, Preflight, Result};
use plumb::skill::{Done, Record, Report};
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
        let (gating, hygiene): (Vec<_>, Vec<_>) =
            audit.faults.iter().partition(|fault| fault.gates());
        if gating.is_empty() {
            println!("  agreement: true to the protocol");
        }
        for fault in gating {
            println!("  {}: {}: {}", fault.kind, fault.path, fault.message);
        }
        if !hygiene.is_empty() {
            println!(
                "  hygiene: {} finding(s); reported, does not gate mutation",
                hygiene.len()
            );
            for fault in hygiene {
                println!("    {}: {}: {}", fault.kind, fault.path, fault.message);
            }
        }
        for resources in &audit.resources {
            audit_report::human(resources);
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

pub fn preflight(preflight: &Preflight, json_output: bool) -> Result<()> {
    if json_output {
        println!(
            "{}",
            serde_json::to_string_pretty(preflight)
                .map_err(|error| Error::new(format!("cannot encode preflight: {error}")))?
        );
    } else {
        human_preflight(preflight);
    }
    if preflight.ok() {
        Ok(())
    } else {
        Err(Error::new(format!(
            "member preflight found {} fault(s) and {} unproved member(s)",
            preflight.faults.len(),
            preflight.unproved()
        )))
    }
}

fn human_preflight(preflight: &Preflight) {
    println!("preflight: {}", preflight.target);
    for member in &preflight.members {
        println!("  member: {}", member.name);
        println!("    path: {}", member.path);
        println!("    source: {}", member.source);
        println!("    expected branch: {}", member.expected_branch);
        if let Some(proof) = &member.proof {
            println!(
                "    identity: {} = {}",
                proof.source_identity, proof.member_identity
            );
            println!("    worktree: registered");
            println!("    branch: {}", proof.branch);
            println!("    clean");
            human_landing(&proof.landing);
        }
    }
    if preflight.ok() {
        println!("  ready to remove landed members");
    }
    for fault in &preflight.faults {
        println!("  {}: {}: {}", fault.kind, fault.path, fault.message);
    }
}

fn human_landing(landing: &LandingProof) {
    match landing {
        LandingProof::Reachable {
            member_head,
            integration_head,
        } => println!(
            "    landed: reachable ({} -> {})",
            member_head, integration_head
        ),
        LandingProof::TreeEquivalent {
            member_tree,
            integration_tree,
            ..
        } => println!(
            "    landed: tree-equivalent ({} = {})",
            member_tree, integration_tree
        ),
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

pub fn skill_report(operation: &str, report: &Report, json_output: bool) -> Result<()> {
    if json_output {
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
