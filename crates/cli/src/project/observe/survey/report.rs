use super::{super::super::fault, inventory::Inventory};
use concord_core::Result;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

pub(super) fn coverage(inventory: &Inventory, projects: &[Value]) -> Result<Value> {
    let mut memberships: BTreeMap<String, Vec<Value>> = BTreeMap::new();
    let mut reports = Vec::new();
    let mut complete = inventory.complete;
    for project in projects {
        let observed = project["complete"] == true;
        complete &= observed;
        let ids = members(project, &mut memberships)?;
        let missing = inventory
            .issues
            .iter()
            .filter(|issue| {
                ordinary(issue) && !ids.contains(issue["id"].as_str().expect("Issue identity"))
            })
            .cloned()
            .collect::<Vec<_>>();
        reports.push(json!({"id":project["project"]["id"],"url":project["project"]["url"],"planning":project["planning"],"complete":observed && inventory.complete,"missing":if observed && inventory.complete {json!(missing)} else {Value::Null}}));
    }
    let mut issues = Vec::new();
    for issue in &inventory.issues {
        let id = issue["id"].as_str().expect("Issue identity");
        let relations = memberships.remove(id).unwrap_or_default();
        confirm(issue, &relations)?;
        issues.push(
            json!({"issue":issue,"classification":classification(issue),"memberships":relations}),
        );
    }
    Ok(
        json!({"complete":complete,"projects":reports,"issues":issues,"counts":{"ordinary":inventory.issues.iter().filter(|issue|ordinary(issue)).count(),"auto":inventory.issues.iter().filter(|issue|classification(issue)=="auto").count(),"unknown":inventory.issues.iter().filter(|issue|classification(issue)=="unknown").count()},"unknown":"unreadable or incomplete scope cannot establish missing membership; attached Issue values and contribution judgments are outside this projection"}),
    )
}

fn confirm(issue: &Value, relations: &[Value]) -> Result<()> {
    for relation in relations {
        for key in [
            "id",
            "number",
            "url",
            "state",
            "updatedAt",
            "issueType",
            "repository",
        ] {
            if issue[key] != relation["basis"][key] {
                return Err(fault(
                    "changed",
                    "shared Issue facts changed between inventory and Project reads",
                ));
            }
        }
    }
    Ok(())
}

fn members(
    project: &Value,
    memberships: &mut BTreeMap<String, Vec<Value>>,
) -> Result<BTreeSet<String>> {
    let mut ids = BTreeSet::new();
    for item in project["items"]["nodes"]
        .as_array()
        .expect("observed items")
    {
        let issue = &item["content"];
        if issue["__typename"] != "Issue" {
            continue;
        }
        let id = issue["id"].as_str().expect("qualified Issue");
        if !ids.insert(id.to_owned()) {
            return Err(fault("changed", "same Issue repeated in a Project"));
        }
        let relation = json!({"project":project["project"]["id"],"item":item["id"],"archived":item["isArchived"],"basis":issue,"values":item["fieldValues"]});
        memberships.entry(id.to_owned()).or_default().push(relation);
    }
    Ok(ids)
}

fn ordinary(issue: &Value) -> bool {
    classification(issue) == "ordinary"
}

fn classification(issue: &Value) -> &'static str {
    match issue["issueType"]["name"].as_str() {
        Some("Feature" | "Task" | "Bug") => "ordinary",
        Some("Auto") => "auto",
        _ => "unknown",
    }
}
