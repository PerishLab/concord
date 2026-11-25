use crate::runtime::github;
use concord_core::{Coordinate, Error, Result};
use runseal::github::Target;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Reply {
    pub node: String,
    pub stable: String,
    coordinate: String,
    branch: String,
    number: i64,
    url: String,
    state: String,
    pub kind: String,
    leaves: usize,
    truncated: bool,
    rulesets: Vec<Ruleset>,
    labels: Labels,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Labels {
    names: Vec<String>,
    truncated: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Ruleset {
    enforcement: String,
    target: String,
    include: Vec<String>,
    exclude: Vec<String>,
    bypass: usize,
    truncated: bool,
    rules: Vec<String>,
}

#[derive(Deserialize)]
struct Raw {
    id: String,
    #[serde(rename = "nameWithOwner")]
    coordinate: String,
    #[serde(rename = "defaultBranchRef")]
    branch: Option<Named>,
    issue: Option<Ticket>,
    rulesets: Connection<Policy>,
}

#[derive(Deserialize)]
struct Named {
    name: String,
}

#[derive(Deserialize)]
struct Ticket {
    id: String,
    number: i64,
    url: String,
    state: String,
    #[serde(rename = "issueType")]
    kind: Option<Named>,
    #[serde(rename = "subIssuesSummary")]
    summary: Summary,
    labels: Connection<Named>,
}

#[derive(Deserialize)]
struct Summary {
    total: usize,
}

#[derive(Deserialize)]
struct Connection<T> {
    nodes: Vec<T>,
    #[serde(rename = "pageInfo")]
    page: Page,
}

#[derive(Deserialize)]
struct Page {
    #[serde(rename = "hasNextPage")]
    more: bool,
}

#[derive(Deserialize)]
struct Policy {
    enforcement: String,
    target: String,
    conditions: Option<Conditions>,
    #[serde(rename = "bypassActors")]
    bypass: Count,
    rules: Connection<Rule>,
}

#[derive(Deserialize)]
struct Conditions {
    #[serde(rename = "refName")]
    refs: Option<Refs>,
}

#[derive(Deserialize)]
struct Refs {
    include: Vec<String>,
    exclude: Vec<String>,
}

#[derive(Deserialize)]
struct Count {
    #[serde(rename = "totalCount")]
    total: usize,
}

#[derive(Deserialize)]
struct Rule {
    #[serde(rename = "type")]
    kind: String,
}

pub(super) async fn observe(issue: &Coordinate, timeout: u64) -> Result<Reply> {
    let client = github::client(timeout).map_err(|error| fault("provider", error))?;
    let number =
        u32::try_from(issue.number).map_err(|_| fault("coordinate", "invalid Issue number"))?;
    let target = Target::new(&issue.owner, &issue.repository, number)
        .map_err(|error| fault("coordinate", github::describe(error)))?;
    let value = github::call("issue.admission", || client.admission(&target))
        .map_err(|error| fault("provider", github::describe(error)))?
        .value;
    let raw: Raw = serde_json::from_value(value)
        .map_err(|_| fault("malformed", "GitHub Issue start reply is malformed"))?;
    let observed = raw
        .issue
        .ok_or_else(|| fault("missing", "GitHub Issue is not readable"))?;
    let reply = Reply {
        node: observed.id,
        stable: raw.id,
        coordinate: raw.coordinate,
        branch: raw.branch.map(|branch| branch.name).unwrap_or_default(),
        number: observed.number,
        url: observed.url,
        state: observed.state,
        kind: observed.kind.map(|kind| kind.name).unwrap_or_default(),
        leaves: observed.summary.total,
        truncated: raw.rulesets.page.more,
        rulesets: raw.rulesets.nodes.into_iter().map(ruleset).collect(),
        labels: Labels {
            names: observed
                .labels
                .nodes
                .into_iter()
                .map(|label| label.name)
                .collect(),
            truncated: observed.labels.page.more,
        },
    };
    shape(issue, reply)
}

fn ruleset(raw: Policy) -> Ruleset {
    let refs = raw.conditions.and_then(|conditions| conditions.refs);
    Ruleset {
        enforcement: raw.enforcement,
        target: raw.target,
        include: refs
            .as_ref()
            .map(|refs| refs.include.clone())
            .unwrap_or_default(),
        exclude: refs.map(|refs| refs.exclude).unwrap_or_default(),
        bypass: raw.bypass.total,
        truncated: raw.rules.page.more,
        rules: raw.rules.nodes.into_iter().map(|rule| rule.kind).collect(),
    }
}

fn shape(issue: &Coordinate, reply: Reply) -> Result<Reply> {
    let expected = format!(
        "https://github.com/{}/{}/issues/{}",
        issue.owner, issue.repository, issue.number
    );
    if reply.node.trim().is_empty() || reply.stable.trim().is_empty() {
        return Err(fault(
            "coordinate",
            "GitHub Issue or repository has no stable identity",
        ));
    }
    if reply.coordinate != format!("{}/{}", issue.owner, issue.repository)
        || reply.number != issue.number
        || reply.url != expected
    {
        return Err(fault(
            "coordinate",
            "GitHub Issue or repository identity does not match the request",
        ));
    }
    if !reply.state.eq_ignore_ascii_case("open")
        || reply.kind.trim().is_empty()
        || reply.leaves != 0
    {
        return Err(fault(
            "issue",
            "Issue start requires one open typed repository-local leaf Issue",
        ));
    }
    if reply.labels.truncated {
        return Err(fault(
            "truncated",
            "GitHub Issue labels observation was truncated",
        ));
    }
    needs(&reply.labels.names)?;
    if reply.branch != "main" {
        return Err(fault(
            "branch",
            "GitHub repository default branch must be main",
        ));
    }
    if reply.truncated || reply.rulesets.iter().any(|ruleset| ruleset.truncated) {
        return Err(fault(
            "truncated",
            "GitHub repository rules observation was truncated",
        ));
    }
    if !reply.rulesets.iter().any(protected) {
        return Err(fault(
            "rules",
            "effective main rules must require pull requests and block deletion and force pushes without bypass",
        ));
    }
    Ok(reply)
}

fn protected(ruleset: &Ruleset) -> bool {
    let required = ["DELETION", "NON_FAST_FORWARD", "PULL_REQUEST"];
    if ruleset.enforcement != "ACTIVE" || ruleset.target != "BRANCH" {
        return false;
    }
    let main = ruleset
        .include
        .iter()
        .any(|value| matches!(value.as_str(), "~DEFAULT_BRANCH" | "refs/heads/main"));
    if !main || !ruleset.exclude.is_empty() || ruleset.bypass != 0 {
        return false;
    }
    required
        .iter()
        .all(|wanted| ruleset.rules.iter().any(|found| found == wanted))
}

fn needs(names: &[String]) -> Result<()> {
    let mut held = names
        .iter()
        .filter(|name| name.starts_with("needs:"))
        .cloned()
        .collect::<Vec<_>>();
    if held.is_empty() {
        return Ok(());
    }
    held.sort();
    held.dedup();
    Err(Error::detailed(
        "concord.issue.needs",
        format!(
            "Issue carries {}; resolve it and remove the label before working it",
            held.join(", ")
        ),
        serde_json::json!({"labels": held}),
    ))
}

fn fault(kind: &str, message: impl Into<String>) -> Error {
    Error::typed(format!("concord.member.observe.{kind}"), message.into())
}
