mod inventory;
mod read;
mod report;

use super::super::{fault, target::Target, wire::Client};
use crate::args::project::Survey;
use concord_core::Result;
use serde_json::{Value, json};
use std::collections::BTreeSet;

pub(in crate::project) async fn run(args: Survey, output: bool) -> Result<()> {
    let targets = targets(&args)?;
    let client = Client {
        command: &args.remote.command,
        timeout: args.remote.timeout,
    };
    let started = super::super::now();
    let day = i64::try_from((started + 8 * 3600) / 86400)
        .map_err(|_| fault("date", "calendar day is outside the supported range"))?;
    let inventory = inventory::read(&client, &args).await?;
    let mut projects = Vec::new();
    for target in targets {
        projects.push(
            read::project(&client, &target, (&inventory.owner, day), args.remote.pages).await?,
        );
    }
    for project in &projects {
        read::finish(&client, &project["project"]).await?;
    }
    let finished = super::super::now();
    if (started + 8 * 3600) / 86400 != (finished + 8 * 3600) / 86400 {
        return Err(fault("changed", "local calendar day changed during survey"));
    }
    let coverage = report::coverage(&inventory, &projects)?;
    let value = json!({
        "schema":"concord.project-survey/v1", "owner":inventory.owner,
        "repositories":inventory.repositories, "issues":inventory.issues,
        "inventory_complete":inventory.complete, "projects":projects,
        "coverage":coverage, "started_at":started, "observed_at":finished,
        "timezone":"Asia/Shanghai", "complete":coverage["complete"],
        "semantics":"provider-visible scope; live interval, not atomic; shared Issue identity counted once; missing membership is not an instruction to import; local field values do not establish contribution, progress, acceptance or permission; attached Issue field values are not read"
    });
    emit(value, output)
}

fn targets(args: &Survey) -> Result<Vec<Target>> {
    Target::parse(&format!("{}/1", args.owner))?;
    if args.projects.len() > 20 {
        return Err(fault(
            "bounds",
            "survey accepts at most twenty explicit Projects",
        ));
    }
    let mut found = BTreeSet::new();
    let mut targets = Vec::new();
    for coordinate in &args.projects {
        let target = Target::parse(coordinate)?;
        if !target.owner.eq_ignore_ascii_case(&args.owner) || !found.insert(target.number) {
            return Err(fault(
                "scope",
                "survey Projects must be distinct and in the selected organization",
            ));
        }
        targets.push(target);
    }
    Ok(targets)
}

fn emit(value: Value, output: bool) -> Result<()> {
    if output {
        crate::output::value(json!({"survey":value,"version":1}), true);
    } else {
        println!(
            "{}: {} observed open Issues; complete={}",
            value["owner"]["login"].as_str().unwrap_or(""),
            value["issues"].as_array().expect("observed issues").len(),
            value["complete"]
        );
        for project in value["coverage"]["projects"]
            .as_array()
            .expect("Project coverage")
        {
            println!(
                "{}: planning={}, absent ordinary candidates={}",
                project["url"].as_str().unwrap_or(""),
                project["planning"]["status"].as_str().unwrap_or("unknown"),
                project["missing"]
                    .as_array()
                    .map(|items| items.len().to_string())
                    .unwrap_or_else(|| "unknown".into())
            );
        }
    }
    Ok(())
}
