use serde_json::{Value, json};

pub(super) fn meta() -> Value {
    let owner = json!({"__typename":"Organization","id":"O_lab","login":"PerishLab"});
    json!({"__typename":"ProjectV2","id":"PVT_plan","number":1,"title":"Planning","url":"https://github.com/orgs/PerishLab/projects/1","updatedAt":"original","closed":false,"public":false,"readme":"Boundary","shortDescription":"Goal","viewerCanUpdate":true,"owner":owner})
}

pub(super) fn connection(nodes: Vec<Value>) -> Value {
    let info = json!({"hasNextPage":false,"endCursor":null});
    json!({"totalCount":nodes.len(),"nodes":nodes,"pageInfo":info})
}

pub(super) fn fields(day: &str) -> Vec<Value> {
    let period = json!({"id":"current","title":"Current","startDate":day,"duration":7});
    let options: Vec<Value> = ["Urgent", "High", "Medium", "Low"]
        .into_iter()
        .map(|name| json!({"id":name,"name":name}))
        .collect();
    let config = json!({"duration":7,"startDay":1,"iterations":[period],"completedIterations":[]});
    let project = json!({"id":"PVT_plan"});
    vec![
        json!({"__typename":"ProjectV2IterationField","id":"F_cycle","name":"Cycle","dataType":"ITERATION","isIssueField":false,"updatedAt":"original","project":project,"configuration":config}),
        json!({"__typename":"ProjectV2SingleSelectField","id":"F_priority","name":"Priority","dataType":"SINGLE_SELECT","isIssueField":false,"updatedAt":"original","project":project,"options":options}),
    ]
}

pub(super) fn issue(index: usize, chosen: usize) -> Value {
    let empty = connection(vec![]);
    let blockers = if index + 1 == chosen {
        connection(vec![
            json!({"id":"I_block","url":"https://github.com/PerishLab/concord/issues/999","state":"OPEN"}),
        ])
    } else {
        empty.clone()
    };
    let labels = if index + 2 == chosen {
        connection(vec![json!({"id":"L_needs","name":"needs:revalidation"})])
    } else {
        empty.clone()
    };
    let repo = json!({"id":"R_repo","nameWithOwner":"PerishLab/concord","owner":{"id":"O_lab"}});
    let kind = json!({"id":"IT_feature","name":"Feature"});
    json!({"__typename":"Issue","id":format!("I_{index}"),"number":index+1,"title":format!("Work {index}"),"url":format!("https://github.com/PerishLab/concord/issues/{}",index+1),"state":"OPEN","body":"Research fixture","updatedAt":"original","issueType":kind,"repository":repo,"parent":null,"blockedBy":blockers,"blocking":empty,"subIssues":empty,"labels":labels,"comments":empty})
}

pub(super) fn item(index: usize, chosen: usize, day: &str) -> Value {
    let cycle = if index + 2 >= chosen {
        "current"
    } else {
        "backlog"
    };
    let priority = if index == chosen { "Low" } else { "Urgent" };
    let values = vec![
        json!({"__typename":"ProjectV2ItemFieldIterationValue","iterationId":cycle,"title":"Current","startDate":day,"duration":7,"field":{"id":"F_cycle"}}),
        json!({"__typename":"ProjectV2ItemFieldSingleSelectValue","optionId":priority,"name":priority,"field":{"id":"F_priority"}}),
    ];
    json!({"id":format!("ITEM_{index}"),"type":"ISSUE","isArchived":false,"updatedAt":"original","project":{"id":"PVT_plan"},"content":issue(index,chosen),"fieldValues":connection(values)})
}

pub(super) fn today() -> String {
    let seconds = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let days = ((seconds + 8 * 3600) / 86400) as i64 + 719_468;
    let era = days.div_euclid(146_097);
    let doe = days.rem_euclid(146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let year = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if month <= 2 { year + 1 } else { year };
    format!("{year:04}-{month:02}-{day:02}")
}
