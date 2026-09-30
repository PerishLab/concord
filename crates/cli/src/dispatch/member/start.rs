use concord_core::{Coordinate, Error, Result};
use serde::Deserialize;
use std::path::Path;
use std::process::Stdio;
use std::time::Duration;
use tokio::process::Command;

const LIMIT: usize = 32 * 1024;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Reply {
    node: String,
    stable: String,
    coordinate: String,
    branch: String,
    number: i64,
    url: String,
    state: String,
    kind: String,
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

pub(super) struct Observation {
    pub node: String,
    pub stable: String,
}

pub(super) async fn observe(
    issue: &Coordinate,
    command: &Path,
    timeout: u64,
) -> Result<Observation> {
    let query = "query($owner:String!,$name:String!,$number:Int!){repository(owner:$owner,name:$name){id nameWithOwner defaultBranchRef{name} issue(number:$number){id number url state issueType{name} subIssuesSummary{total} labels(first:100){nodes{name} pageInfo{hasNextPage}}} rulesets(first:100,includeParents:true){nodes{enforcement target conditions{refName{include exclude}} bypassActors(first:100){totalCount} rules(first:100){nodes{type}pageInfo{hasNextPage}}}pageInfo{hasNextPage}}}}";
    let selector = ".data.repository as $r | $r.issue as $i | if $r == null or $i == null then null else {node:$i.id,stable:$r.id,coordinate:$r.nameWithOwner,branch:($r.defaultBranchRef.name // \"\"),number:$i.number,url:$i.url,state:$i.state,kind:($i.issueType.name // \"\"),leaves:$i.subIssuesSummary.total,truncated:$r.rulesets.pageInfo.hasNextPage,rulesets:[$r.rulesets.nodes[]|{enforcement:.enforcement,target:.target,include:(.conditions.refName.include // []),exclude:(.conditions.refName.exclude // []),bypass:.bypassActors.totalCount,truncated:.rules.pageInfo.hasNextPage,rules:[.rules.nodes[].type]}],labels:{names:[$i.labels.nodes[].name],truncated:$i.labels.pageInfo.hasNextPage}} end";
    let mut process = Command::new(command);
    process
        .args(["api", "graphql", "-f"])
        .arg(format!("query={query}"))
        .args(["-f", &format!("owner={}", issue.owner)])
        .args(["-f", &format!("name={}", issue.repository)])
        .args(["-F", &format!("number={}", issue.number)])
        .args(["--jq", selector])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true);
    let output = match tokio::time::timeout(Duration::from_secs(timeout), process.output()).await {
        Ok(Ok(output)) => output,
        Ok(Err(error)) => return Err(fault("command", error.to_string())),
        Err(_) => return Err(fault("timeout", "GitHub Issue start observation timed out")),
    };
    if !output.status.success() {
        return Err(fault(
            "provider",
            "GitHub Issue start observation was refused by the provider",
        ));
    }
    if output.stdout.len() > LIMIT {
        return Err(fault(
            "malformed",
            "GitHub Issue start observation exceeds the reply limit",
        ));
    }
    let reply = serde_json::from_slice::<Option<Reply>>(&output.stdout)
        .map_err(|_| fault("malformed", "GitHub Issue start reply is not valid JSON"))?
        .ok_or_else(|| fault("missing", "GitHub Issue is not readable"))?;
    shape(issue, reply)
}

fn shape(issue: &Coordinate, reply: Reply) -> Result<Observation> {
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
    Ok(Observation {
        node: reply.node,
        stable: reply.stable,
    })
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
