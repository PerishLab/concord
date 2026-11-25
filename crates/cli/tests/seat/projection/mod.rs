#[cfg(unix)]
mod arrange;
#[cfg(unix)]
mod confirmation;
#[cfg(unix)]
mod managed;
#[cfg(unix)]
mod management;
mod observed;
#[cfg(unix)]
mod planning;
mod project;
#[cfg(unix)]
mod snapshot;
mod survey;

#[cfg(unix)]
mod unix {
    use super::super::spawn::{self, Reply, Request, Server};
    use serde_json::{Value, json};
    use std::path::Path;
    use std::process::Output;

    pub(super) const PLUMB: &str = "[release]\nauthority = \"https://releases.plumb.perish.uk\"\n";

    #[test]
    fn projections() {
        let fixture = tempfile::tempdir().expect("fixture");
        success(fixture.path(), &["issue", "bootstrap"]);
        let server = serve(Some(PLUMB));
        let brief = attached(
            fixture.path(),
            &server,
            &[
                "issue",
                "brief",
                "PerishLab/concord#27",
                "--page-size",
                "1",
                "--blocking-after",
                "exact-cursor",
            ],
        );
        assert_eq!(brief["brief"]["schema"], "concord.issue-brief/v1");
        assert_eq!(brief["brief"]["kind"], "Feature");
        assert_eq!(
            brief["brief"]["blocking"]["requested_after"],
            "exact-cursor"
        );
        assert_eq!(brief["brief"]["pulls"]["nodes"][0]["number"], 32);

        let graph = attached(
            fixture.path(),
            &server,
            &["issue", "graph", "PerishLab/concord#27", "--max-nodes", "4"],
        );
        assert_eq!(graph["graph"]["schema"], "concord.issue-graph/v1");
        assert_eq!(graph["graph"]["complete"], true);
        assert_eq!(graph["graph"]["nodes"].as_array().expect("nodes").len(), 1);

        let ready = attached(
            fixture.path(),
            &server,
            &["issue", "ready", "PerishLab/concord#27"],
        );
        assert_eq!(ready["readiness"]["schema"], "concord.issue-readiness/v1");
        assert_eq!(ready["readiness"]["ready"], true);
        assert_eq!(ready["readiness"]["checks"]["acceptance_total"], 1);
        assert_eq!(
            ready["readiness"]["distribution_evidence"][0],
            "https://releases.plumb.perish.uk/v1/releases/stable/v1/distribution.json"
        );
        assert_eq!(
            ready["readiness"]["distribution_evidence"]
                .as_array()
                .expect("evidence")
                .len(),
            1
        );
        unavailable(fixture.path());
        withheld(fixture.path(), None, "concord.issue.projection.provider");
        let invalid = "[release]\nauthority = \"http://releases.plumb.perish.uk/\"\n";
        withheld(
            fixture.path(),
            Some(invalid),
            "concord.issue.projection.declaration",
        );
    }

    fn withheld(space: &Path, declaration: Option<&str>, code: &str) {
        let server = serve(declaration);
        let ready = raw(
            space,
            Some(&server),
            &["issue", "ready", "PerishLab/concord#27"],
        );
        assert!(!ready.status.success());
        assert!(String::from_utf8_lossy(&ready.stderr).contains(code));
    }

    fn unavailable(space: &Path) {
        let server = Server::start(|_| Reply::status(403, json!({"message": "forbidden"})));
        let brief = raw(
            space,
            Some(&server),
            &["issue", "brief", "PerishLab/concord#27"],
        );
        assert!(!brief.status.success());
        assert!(
            String::from_utf8_lossy(&brief.stderr).contains("concord.issue.projection.provider")
        );
        let graph = attached(space, &server, &["issue", "graph", "PerishLab/concord#27"]);
        assert_eq!(graph["graph"]["complete"], false);
        assert_eq!(graph["graph"]["diagnostics"][0]["code"], "provider");
        assert!(
            graph["graph"]["nodes"]
                .as_array()
                .expect("nodes")
                .is_empty()
        );
    }

    pub(super) fn serve(declaration: Option<&str>) -> Server {
        let declaration = declaration.map(str::to_owned);
        Server::start(move |request| handle(request, declaration.as_deref()))
    }

    fn handle(request: Request, declaration: Option<&str>) -> Reply {
        let query = request.query();
        if query.contains("RunsealManifest") {
            return manifest(&request.variables(), declaration);
        }
        if query.contains("RunsealAcceptancePage") || query.contains("RunsealAcceptanceRelations") {
            return Reply::json(strict());
        }
        Reply::json(snapshot())
    }

    fn manifest(variables: &Value, declaration: Option<&str>) -> Reply {
        let Some(declaration) = declaration else {
            return Reply::status(403, json!({"message": "forbidden"}));
        };
        let name = variables["name"].as_str().unwrap_or("");
        let (id, text) = if name == "plumb" {
            ("R_plumb", declaration)
        } else {
            ("R_concord", "[package]\nname = \"concord\"\n")
        };
        let object = json!({"oid": "a1b2c3", "text": text});
        let repository = json!({"id": id, "object": object});
        Reply::json(json!({"data": {"repository": repository}}))
    }

    fn snapshot() -> Value {
        let issue = json!({
            "id": "I_issue",
            "number": 27,
            "url": "https://github.com/PerishLab/concord/issues/27",
            "title": "Bounded projection",
            "body": "## Problem\nLedger drift\n\n## Outcome\nProject GitHub\n\n## Acceptance\n- [x] bounded\n\n## Non-goals\nMutation\n",
            "state": "OPEN",
            "updatedAt": "2026-09-28T00:00:00Z",
            "issueType": {"name": "Feature"},
            "repository": {"nameWithOwner": "PerishLab/concord"},
            "parent": null,
            "subIssues": connection(Vec::<Value>::new()),
            "subIssuesSummary": {"total": 0, "completed": 0, "percentCompleted": 0},
            "blockedBy": connection(Vec::<Value>::new()),
            "blocking": connection(Vec::<Value>::new()),
            "timelineItems": connection(vec![pull()]),
            "comments": connection(vec![comment()]),
        });
        let repository =
            json!({"id": "R_concord", "nameWithOwner": "PerishLab/concord", "issue": issue});
        json!({"data": {"repository": repository}})
    }

    fn strict() -> Value {
        let empty = json!({"total": 0, "rows": [], "page": {"more": false, "cursor": null}});
        let issue = json!({
            "id": "I_issue",
            "number": 27,
            "title": "Bounded projection",
            "body": "## Problem\nLedger drift\n\n## Outcome\nProject GitHub\n\n## Acceptance\n- [x] bounded\n\n## Non-goals\nMutation\n",
            "url": "https://github.com/PerishLab/concord/issues/27",
            "state": "OPEN",
            "reason": null,
            "updated": "2026-09-28T00:00:00Z",
            "writable": true,
            "kind": {"name": "Feature"},
            "repository": {"id": "R_concord"},
            "labels": empty,
            "comments": empty,
            "children": empty,
            "blockers": empty,
        });
        let repository =
            json!({"id": "R_concord", "nameWithOwner": "PerishLab/concord", "issue": issue});
        json!({"data": {"repository": repository}})
    }

    fn pull() -> Value {
        let source = json!({
            "__typename": "PullRequest",
            "id": "PR_pull",
            "number": 32,
            "url": "https://github.com/PerishLab/plumb/pull/32",
            "title": "Deliver projection",
            "state": "MERGED",
            "mergedAt": "2026-09-28T00:00:00Z",
            "updatedAt": "2026-09-28T00:00:00Z",
            "repository": {"nameWithOwner": "PerishLab/plumb"}
        });
        json!({"source": source})
    }

    fn comment() -> Value {
        json!({
            "body": "Released https://releases.plumb.perish.uk/v1/releases/stable/v1/distribution.json and https://releases.example.com/v1/releases/stable/v1/distribution.json",
            "url": "https://github.com/PerishLab/concord/issues/27#issuecomment-1"
        })
    }

    fn connection(nodes: Vec<Value>) -> Value {
        json!({
            "totalCount": nodes.len(),
            "pageInfo": {"hasNextPage": false, "endCursor": "exact-end"},
            "nodes": nodes,
        })
    }

    pub(super) fn success(space: &Path, arguments: &[&str]) -> Value {
        let output = raw(space, None, arguments);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        serde_json::from_slice(&output.stdout).expect("Concord JSON")
    }

    fn attached(space: &Path, server: &Server, arguments: &[&str]) -> Value {
        let output = raw(space, Some(server), arguments);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        serde_json::from_slice(&output.stdout).expect("Concord JSON")
    }

    pub(super) fn raw(space: &Path, server: Option<&Server>, arguments: &[&str]) -> Output {
        let mut command = spawn::concord(space);
        command
            .args(["--home", space.to_str().expect("root path"), "--json"])
            .args(arguments);
        if let Some(server) = server {
            server.attach(&mut command);
        }
        command.output().expect("run Concord")
    }
}
