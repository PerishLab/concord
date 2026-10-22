pub(in crate::project) const META: &str = "__typename id number title url updatedAt closed public readme shortDescription viewerCanUpdate owner{__typename ... on Organization{id login}}";

const REFERENCE: &str = "__typename ... on ProjectV2FieldCommon{id name dataType isIssueField}";

pub(in crate::project) const PAGE: &str = "totalCount pageInfo{hasNextPage endCursor}";

pub(in crate::project) const FIELD: &str = r#"
__typename
... on ProjectV2FieldCommon{id name dataType isIssueField updatedAt project{id}}
... on ProjectV2Field{issueField{
... on IssueFieldText{id fullDatabaseId name dataType}
... on IssueFieldNumber{id fullDatabaseId name dataType}
... on IssueFieldDate{id fullDatabaseId name dataType}
... on IssueFieldSingleSelect{id fullDatabaseId name dataType options{id name color description}}
... on IssueFieldMultiSelect{id fullDatabaseId name dataType}
}}
... on ProjectV2SingleSelectField{issueField{
... on IssueFieldText{id fullDatabaseId name dataType}
... on IssueFieldNumber{id fullDatabaseId name dataType}
... on IssueFieldDate{id fullDatabaseId name dataType}
... on IssueFieldSingleSelect{id fullDatabaseId name dataType options{id name color description}}
... on IssueFieldMultiSelect{id fullDatabaseId name dataType}
} options{id name color description}}
... on ProjectV2MultiSelectField{issueField{
... on IssueFieldText{id fullDatabaseId name dataType}
... on IssueFieldNumber{id fullDatabaseId name dataType}
... on IssueFieldDate{id fullDatabaseId name dataType}
... on IssueFieldSingleSelect{id fullDatabaseId name dataType options{id name color description}}
... on IssueFieldMultiSelect{id fullDatabaseId name dataType}
} multiSelectOptions{id name color description}}
... on ProjectV2IterationField{configuration{duration startDay iterations{id title startDate duration} completedIterations{id title startDate duration}}}
"#;

const ISSUE: &str = r#"
id number title url state body updatedAt issueType{id name} repository{id nameWithOwner}
parent{id url}
blockedBy(first:100){totalCount pageInfo{hasNextPage endCursor} nodes{id url state}}
blocking(first:100){totalCount pageInfo{hasNextPage endCursor} nodes{id url state}}
subIssues(first:100){totalCount pageInfo{hasNextPage endCursor} nodes{id url state}}
labels(first:100){totalCount pageInfo{hasNextPage endCursor} nodes{id name}}
comments(first:100){totalCount pageInfo{hasNextPage endCursor} nodes{id url body updatedAt}}
"#;

const VALUE: &str = r#"
__typename
... on ProjectV2ItemFieldTextValue{id text field{... on ProjectV2FieldCommon{id}}}
... on ProjectV2ItemFieldNumberValue{id number field{... on ProjectV2FieldCommon{id}}}
... on ProjectV2ItemFieldDateValue{id date field{... on ProjectV2FieldCommon{id}}}
... on ProjectV2ItemFieldSingleSelectValue{id optionId name field{... on ProjectV2FieldCommon{id}}}
... on ProjectV2ItemFieldIterationValue{id iterationId title startDate duration field{... on ProjectV2FieldCommon{id}}}
... on ProjectV2ItemFieldMultiSelectValue{id value options{id name} field{... on ProjectV2FieldCommon{id}}}
... on ProjectV2ItemFieldRepositoryValue{field{... on ProjectV2FieldCommon{id}} repository{id nameWithOwner url}}
... on ProjectV2ItemFieldMilestoneValue{field{... on ProjectV2FieldCommon{id}} milestone{id title state url}}
... on ProjectV2ItemFieldLabelValue{field{... on ProjectV2FieldCommon{id}} labels(first:100){totalCount pageInfo{hasNextPage endCursor} nodes{id name}}}
... on ProjectV2ItemFieldUserValue{field{... on ProjectV2FieldCommon{id}} users(first:100){totalCount pageInfo{hasNextPage endCursor} nodes{id login}}}
... on ProjectV2ItemFieldPullRequestValue{field{... on ProjectV2FieldCommon{id}} pullRequests(first:100){totalCount pageInfo{hasNextPage endCursor} nodes{id url state}}}
... on ProjectV2ItemFieldReviewerValue{field{... on ProjectV2FieldCommon{id}} reviewers(first:100){totalCount pageInfo{hasNextPage endCursor} nodes{__typename ... on Node{id}}}}
... on ProjectV2ItemIssueFieldValue{field{... on ProjectV2FieldCommon{id}} issueFieldValue{
__typename
... on IssueFieldTextValue{id value field{... on Node{id}}}
... on IssueFieldNumberValue{id value field{... on Node{id}}}
... on IssueFieldDateValue{id value field{... on Node{id}}}
... on IssueFieldSingleSelectValue{id value optionId name field{... on Node{id}}}
... on IssueFieldMultiSelectValue{id value field{... on Node{id}}}
}}
"#;

pub(in crate::project) fn selection(name: &str) -> String {
    match name {
        "fields" => FIELD.into(),
        "items" => format!(
            "id type isArchived updatedAt project{{id}} fieldValues(first:100){{{PAGE} nodes{{{VALUE}}}}} content{{__typename ... on Issue{{{ISSUE}}} ... on PullRequest{{id number title url state updatedAt repository{{id nameWithOwner}}}} ... on DraftIssue{{id title body updatedAt}}}}"
        ),
        "views" => format!(
            "id number name layout filter updatedAt project{{id}} configuration{{visibleFields(first:100){{pageInfo{{hasNextPage endCursor}} nodes{{{REFERENCE}}}}}}} groupByFields(first:100){{{PAGE} nodes{{{REFERENCE}}}}} verticalGroupByFields(first:100){{{PAGE} nodes{{{REFERENCE}}}}} sortByFields(first:100){{{PAGE} nodes{{direction field{{{REFERENCE}}}}}}}"
        ),
        "workflows" => "id number name enabled updatedAt project{id}".into(),
        "repositories" => "id nameWithOwner url".into(),
        "statusUpdates" => "id body createdAt updatedAt startDate targetDate status".into(),
        _ => unreachable!("fixed Project connection"),
    }
}

pub(in crate::project) fn connection(name: &str) -> String {
    format!(
        "query($id:ID!,$size:Int!,$after:String){{node(id:$id){{... on ProjectV2{{{META} {name}(first:$size,after:$after){{{PAGE} nodes{{{selection}}}}}}}}}}}",
        selection = selection(name)
    )
}
