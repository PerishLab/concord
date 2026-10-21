mod cycle;
mod date;
mod desired;
mod facts;
mod model;
mod scope;
mod steps;

pub(super) use desired::result;
pub(super) use model::Change;
pub(super) use scope::{qualify, summary};
pub(super) use steps::build;

pub(super) fn selection(item: &str) -> String {
    let kinds = [
        "Date",
        "Label",
        "Milestone",
        "MultiSelect",
        "Number",
        "PullRequest",
        "Repository",
        "Reviewer",
        "Text",
        "User",
    ];
    let references = kinds
        .iter()
        .map(|kind| {
            format!(
                "... on ProjectV2ItemField{kind}Value{{field{{... on ProjectV2FieldCommon{{id}}}}}}"
            )
        })
        .collect::<Vec<_>>()
        .join(" ");
    format!(
        "{item} fieldValues(first:100){{totalCount pageInfo{{hasNextPage endCursor}} nodes{{__typename {references} ... on ProjectV2ItemIssueFieldValue{{field{{... on ProjectV2FieldCommon{{id}}}}}} ... on ProjectV2ItemFieldSingleSelectValue{{optionId name field{{... on ProjectV2FieldCommon{{id}}}}}} ... on ProjectV2ItemFieldIterationValue{{iterationId title startDate duration field{{... on ProjectV2FieldCommon{{id}}}}}}}}}}"
    )
}
