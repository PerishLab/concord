use super::super::super::spawn::{Reply, Request};
use super::graph::Config;
use serde_json::{Value, json};
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

static CLOCK: AtomicU64 = AtomicU64::new(1_700_000_000);

struct State {
    status: &'static str,
    merged: bool,
    oid: Value,
    landed: Value,
}

impl Config {
    pub(super) fn slow(&self, request: &Request) {
        let pull = format!("/repos/{}/pulls/7", self.coordinate);
        if request.method == "GET"
            && request.path == pull
            && self.root.join("slow-readback").exists()
        {
            std::thread::sleep(Duration::from_secs(1));
        }
    }

    pub(super) fn rest(&self, request: &Request) -> Reply {
        let (route, query) = request
            .path
            .split_once('?')
            .unwrap_or((request.path.as_str(), ""));
        let prefix = format!("/repos/{}", self.coordinate);
        let pulls = format!("{prefix}/pulls");
        let pull = format!("{pulls}/7");
        let merge = format!("{pull}/merge");
        let statuses = format!("{prefix}/statuses/");
        match (request.method.as_str(), route) {
            ("GET", path) if path == prefix => self.repository(),
            ("GET", path) if path == pulls => self.listing(query),
            ("POST", path) if path == pulls => self.create(request),
            ("GET", path) if path == pull => self.show(),
            ("PATCH", path) if path == pull => self.edit(request),
            ("PUT", path) if path == merge => self.squash(request),
            ("POST", path) if path.starts_with(&statuses) => {
                self.mark(&path[statuses.len()..], request)
            }
            _ => Reply::status(404, json!({"message": "unknown rest route"})),
        }
    }

    fn repository(&self) -> Reply {
        Reply::json(
            json!({"id": 1, "node_id": self.stable, "full_name": self.coordinate, "private": false, "default_branch": "main"}),
        )
    }

    fn listing(&self, query: &str) -> Reply {
        if query.contains("base=") {
            if self.root.join("pull-state").exists() {
                return Reply::json(json!([self.pull()]));
            }
            return Reply::json(json!([]));
        }
        self.projections()
    }

    fn projections(&self) -> Reply {
        let Ok(bytes) = std::fs::read(self.root.join("branch-projections")) else {
            return Reply::json(json!([]));
        };
        let Ok(value) = serde_json::from_slice::<Value>(&bytes) else {
            return Reply::raw(200, bytes);
        };
        if let Some(status) = value.get("status").and_then(Value::as_u64) {
            return Reply::status(status as u16, value["body"].clone());
        }
        let Some(body) = value.get("body") else {
            return Reply::json(value);
        };
        let reply = Reply::json(body.clone());
        if value.get("next").and_then(Value::as_bool) == Some(true) {
            return reply.header("Link", "<https://api.github.com/next>; rel=\"next\"");
        }
        reply
    }

    fn create(&self, request: &Request) -> Reply {
        let fail = self.root.join("fail-create");
        if fail.exists() {
            let _ = std::fs::remove_file(fail);
            return Reply::status(500, json!({"message": "create-failed"}));
        }
        let body = request.json();
        write(
            &self.root,
            "pull-base",
            body["base"].as_str().unwrap_or("main"),
        );
        write(&self.root, "pull-head", body["head"].as_str().unwrap_or(""));
        write(
            &self.root,
            "pull-title",
            body["title"].as_str().unwrap_or(""),
        );
        write(&self.root, "pull-body", body["body"].as_str().unwrap_or(""));
        write(&self.root, "pull-state", "OPEN");
        let _ = std::fs::remove_file(self.root.join("pull-merge"));
        let _ = std::fs::remove_file(self.root.join("pull-merge.head"));
        Reply::status(201, self.pull())
    }

    fn show(&self) -> Reply {
        let hidden = self.root.join("hide-readback");
        if hidden.exists() && read(&self.root, "pull-state").as_deref() == Some("MERGED") {
            let _ = std::fs::remove_file(hidden);
            return Reply::status(500, json!({"message": "hidden-readback"}));
        }
        Reply::json(self.pull())
    }

    fn edit(&self, request: &Request) -> Reply {
        let body = request.json();
        if let Some(title) = body["title"].as_str() {
            write(&self.root, "pull-title", title);
        }
        if let Some(text) = body["body"].as_str() {
            write(&self.root, "pull-body", text);
        }
        Reply::json(self.pull())
    }

    fn squash(&self, request: &Request) -> Reply {
        append(&self.root.join("merge-attempts"), "merge\n");
        let body = request.json();
        let sha = body["sha"].as_str().unwrap_or("").to_string();
        if self.root.join("incomplete").exists() {
            return Reply::json(json!({"sha": sha, "merged": true}));
        }
        let waiting = read(&self.root, "checks-pending")
            .and_then(|text| text.trim().parse::<u64>().ok())
            .unwrap_or(0);
        if waiting > 0 || self.root.join("checks-rollup").exists() {
            return Reply::status(405, json!({"message": "not mergeable"}));
        }
        let subject = body["commit_title"].as_str().unwrap_or("");
        let message = if self.root.join("lose-proof").exists() {
            "receipt missing"
        } else {
            body["commit_message"].as_str().unwrap_or("")
        };
        let Some(created) = commit(&self.root.join("remote.git"), &sha, subject, message) else {
            return Reply::status(500, json!({"message": "commit-failed"}));
        };
        write(&self.root, "pull-merge", &created);
        write(&self.root, "pull-merge.head", &sha);
        write(&self.root, "pull-state", "MERGED");
        Reply::json(json!({"sha": created, "merged": true}))
    }

    fn mark(&self, sha: &str, request: &Request) -> Reply {
        let body = request.json();
        let context = body["context"].as_str().unwrap_or("");
        append(&self.root.join("gate-marks"), "mark\n");
        write(&self.root, "status-call", &format!("context={context}\n"));
        Reply::status(
            201,
            json!({"id": 1, "state": body["state"], "context": body["context"], "description": body["description"], "target_url": body["target_url"], "url": format!("https://api.github.com/repos/{}/statuses/{sha}", self.coordinate)}),
        )
    }

    fn pull(&self) -> Value {
        let base = read(&self.root, "pull-base").unwrap_or_else(|| "main".into());
        let head = read(&self.root, "pull-head").unwrap_or_default();
        let title = read(&self.root, "pull-title").unwrap_or_default();
        let body = read(&self.root, "pull-body").unwrap_or_default();
        let state = state(&self.root);
        let origin = json!({"id": 1, "node_id": self.stable, "full_name": self.coordinate});
        let target = rev(&self.root.join("remote.git"), &format!("refs/heads/{base}"))
            .unwrap_or_else(|| "0".repeat(40));
        json!({
            "id": 700, "number": 7, "node_id": "PR_node",
            "html_url": format!("https://github.com/{}/pull/7", self.coordinate),
            "title": title, "body": body, "state": state.status, "draft": false,
            "merged": state.merged, "merge_commit_sha": state.oid,
            "merged_at": state.landed, "updated_at": "2026-09-29T00:00:03Z",
            "head": {"ref": head, "sha": self.candidate(), "repo": origin.clone()},
            "base": {"ref": base, "sha": target, "repo": origin},
        })
    }

    pub(super) fn candidate(&self) -> String {
        if let Some(sha) = read(&self.root, "pull-merge.head")
            && !sha.trim().is_empty()
        {
            return sha.trim().to_string();
        }
        let branch = read(&self.root, "pull-head").unwrap_or_default();
        rev(
            &self.root.join("remote.git"),
            &format!("refs/heads/{branch}"),
        )
        .unwrap_or_else(|| "0".repeat(40))
    }
}

fn state(root: &Path) -> State {
    let held = read(root, "pull-state").unwrap_or_else(|| "OPEN".into());
    match held.as_str() {
        "MERGED" => State {
            status: "closed",
            merged: true,
            oid: json!(read(root, "pull-merge").unwrap_or_default()),
            landed: json!("2026-09-29T00:00:02Z"),
        },
        "CLOSED" => State {
            status: "closed",
            merged: false,
            oid: Value::Null,
            landed: Value::Null,
        },
        _ => State {
            status: "open",
            merged: false,
            oid: Value::Null,
            landed: Value::Null,
        },
    }
}

fn commit(remote: &Path, candidate: &str, subject: &str, message: &str) -> Option<String> {
    let tree = rev(remote, &format!("{candidate}^{{tree}}"))?;
    let parent = rev(remote, "refs/heads/main")?;
    let moment = format!("{} +0000", CLOCK.fetch_add(1, Ordering::Relaxed));
    let mut child = Command::new("git")
        .env("GIT_DIR", remote)
        .env("GIT_AUTHOR_DATE", &moment)
        .env("GIT_COMMITTER_DATE", &moment)
        .args([
            "-c",
            "user.name=GitHub",
            "-c",
            "user.email=noreply@github.com",
        ])
        .args(["commit-tree", &tree, "-p", &parent])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .ok()?;
    child
        .stdin
        .take()?
        .write_all(format!("{subject}\n\n{message}\n").as_bytes())
        .ok()?;
    let output = child.wait_with_output().ok()?;
    if !output.status.success() {
        return None;
    }
    let created = String::from_utf8(output.stdout).ok()?.trim().to_string();
    let updated = Command::new("git")
        .env("GIT_DIR", remote)
        .args(["update-ref", "refs/heads/main", &created])
        .status()
        .ok()?;
    updated.success().then_some(created)
}

fn rev(remote: &Path, reference: &str) -> Option<String> {
    let output = Command::new("git")
        .env("GIT_DIR", remote)
        .args(["rev-parse", reference])
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_string())
}

pub(in super::super) fn append(path: &Path, line: &str) {
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .expect("append state");
    let _ = file.write_all(line.as_bytes());
}

fn read(root: &Path, name: &str) -> Option<String> {
    std::fs::read_to_string(root.join(name)).ok()
}

fn write(root: &Path, name: &str, content: &str) {
    let _ = std::fs::write(root.join(name), content);
}
