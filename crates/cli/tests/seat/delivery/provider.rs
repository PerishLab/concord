use serde_json::{Value, json};
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output, Stdio};

use super::super::spawn;

pub fn consume(space: &Path, provider: &str, plan: &[u8]) -> Output {
    start(space, provider, plan)
        .wait_with_output()
        .expect("complete land")
}

pub fn start(space: &Path, provider: &str, plan: &[u8]) -> Child {
    let mut consumer = land(space, provider)
        .stdin(Stdio::piped())
        .spawn()
        .expect("consume stored plan");
    consumer
        .stdin
        .take()
        .expect("land stdin")
        .write_all(plan)
        .expect("write stored plan");
    consumer
}

fn land(space: &Path, provider: &str) -> Command {
    let mut command = spawn::concord(space);
    command
        .args(["--root", space.to_str().expect("root"), "--json"])
        .args([
            "issue",
            "delivery",
            "land",
            "PerishLab/probe#1",
            "--plan",
            "-",
            "--github-command",
            provider,
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    command
}

pub fn observer(root: &Path) -> PathBuf {
    let reply = json!({
        "node": "R_probe",
        "coordinate": "PerishLab/probe",
        "branch": "main",
    });
    let path = root.join("repository-provider");
    std::fs::write(&path, format!("#!/bin/sh\nprintf '%s\\n' '{reply}'\n"))
        .expect("repository provider");
    let mut permissions = std::fs::metadata(&path).expect("metadata").permissions();
    permissions.set_mode(0o700);
    std::fs::set_permissions(&path, permissions).expect("provider mode");
    path
}

pub fn projection(root: &Path) -> String {
    let observed = json!({
        "node": "I_delivery", "stable": "R_probe", "number": 1,
        "url": "https://github.com/PerishLab/probe/issues/1",
        "state": "OPEN", "kind": "Task", "updated_at": "2026-09-29T00:00:00Z",
    });
    let issue = json!({
        "id": "I_delivery", "number": 1,
        "url": "https://github.com/PerishLab/probe/issues/1",
        "title": "Stream delivery plan", "body": "## Outcome\nPipe exact plan",
        "state": "OPEN", "updatedAt": "2026-09-29T00:00:00Z",
        "issueType": {"name": "Task"},
        "repository": {"nameWithOwner": "PerishLab/probe"}, "parent": null,
        "subIssues": connection(),
        "subIssuesSummary": {"total": 0, "completed": 0, "percentCompleted": 0},
        "blockedBy": connection(), "blocking": connection(),
        "timelineItems": connection(), "comments": connection(),
    });
    let projected = json!({"data": {"repository": {"issue": issue}}});
    let remote = root.join("remote.git");
    let state = root.join("pull-state");
    let merge = root.join("pull-merge");
    let title = root.join("pull-title");
    let body = root.join("pull-body");
    let head = root.join("pull-head");
    let base = root.join("pull-base");
    let incomplete = root.join("incomplete");
    let hidden = root.join("hide-readback");
    let slow = root.join("slow-readback");
    let create = root.join("fail-create");
    format!(
        r#"reply() {{
  state=$(cat '{state}')
  candidate=$(git --git-dir='{remote}' rev-parse "refs/heads/$(cat '{head}')") || exit 1
  merge=$(cat '{merge}' 2>/dev/null || true)
  jq -cn --arg id PR_node --argjson number 7 --arg url https://github.com/PerishLab/probe/pull/7 --arg state "$state" --arg base "$(cat '{base}')" --arg head "$candidate" --arg merge "$merge" --arg title "$(cat '{title}')" --arg body "$(cat '{body}')" '{{id:$id,number:$number,url:$url,state:$state,baseRefName:$base,headRefOid:$head,mergeCommit:(if $merge == "" then null else {{oid:$merge}} end),mergedAt:(if $merge == "" then null else "2026-09-29T00:00:02Z" end),updatedAt:"2026-09-29T00:00:03Z",title:$title,body:$body}}'
}}
if [ "$1 $2" = "api graphql" ]; then
  for argument in "$@"; do [ "$argument" = "--jq" ] && printf '%s\n' '{observed}' && exit 0; done
  printf '%s\n' '{projected}'
  exit 0
fi
if [ "$1 $2" = "api -X" ]; then printf '%s\n' '{{}}'; exit 0; fi
if [ "$1 $2" = "pr list" ]; then
  [ -f '{state}' ] && reply | jq -s . || printf '%s\n' '[]'
  exit 0
fi
if [ "$1 $2" = "pr create" ]; then
  if [ -f '{create}' ]; then rm '{create}'; printf '%s\n' create-failed >&2; exit 1; fi
  shift 2
  while [ $# -gt 0 ]; do
    case "$1" in
      --base) printf '%s' "$2" > '{base}'; shift ;;
      --head) printf '%s' "$2" > '{head}'; shift ;;
      --title) printf '%s' "$2" > '{title}'; shift ;;
      --body) printf '%s' "$2" > '{body}'; shift ;;
    esac
    shift
  done
  printf '%s' OPEN > '{state}'
  printf '%s\n' https://github.com/PerishLab/probe/pull/7
  exit 0
fi
if [ "$1 $2" = "pr view" ]; then
  if [ -f '{hidden}' ] && [ "$(cat '{state}')" = MERGED ]; then rm '{hidden}'; printf '%s\n' hidden-readback >&2; exit 1; fi
  [ -f '{slow}' ] && sleep 1
  reply
  exit 0
fi
if [ "$1 $2" = "pr merge" ]; then
  [ -f '{incomplete}' ] && exit 0
  shift 3
  while [ $# -gt 0 ]; do
    case "$1" in
      --match-head-commit) candidate=$2; shift ;;
      --subject) subject=$2; shift ;;
      --body) message=$2; shift ;;
    esac
    shift
  done
  export GIT_DIR='{remote}'
  tree=$(git rev-parse "$candidate^{{tree}}") || exit 1
  parent=$(git rev-parse refs/heads/main) || exit 1
  commit=$(printf '%s\n\n%s\n' "$subject" "$message" | git -c user.name=GitHub -c user.email=noreply@github.com commit-tree "$tree" -p "$parent") || exit 1
  git update-ref refs/heads/main "$commit" || exit 1
  printf '%s' "$commit" > '{merge}'
  printf '%s' MERGED > '{state}'
  exit 0
fi
printf '%s\n' unsupported-provider-command >&2
exit 1"#,
        remote = remote.display(),
        state = state.display(),
        merge = merge.display(),
        title = title.display(),
        body = body.display(),
        head = head.display(),
        base = base.display(),
        incomplete = incomplete.display(),
        hidden = hidden.display(),
        slow = slow.display(),
        create = create.display(),
    )
}

fn connection() -> Value {
    json!({
        "totalCount": 0,
        "pageInfo": {"hasNextPage": false, "endCursor": null},
        "nodes": [],
    })
}

pub fn tool(root: &Path, name: &str, body: String) -> PathBuf {
    let path = root.join(name);
    std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).expect("provider");
    let mut permissions = std::fs::metadata(&path).expect("metadata").permissions();
    permissions.set_mode(0o700);
    std::fs::set_permissions(&path, permissions).expect("provider mode");
    path
}
