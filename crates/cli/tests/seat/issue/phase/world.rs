use super::mock::serve;
use crate::spawn::{self, Server};
use serde_json::{Value, json};
use std::process::Output;
use std::sync::{Arc, Mutex};

pub(super) struct World {
    pub(super) root: tempfile::TempDir,
    server: Server,
    state: Arc<Mutex<Value>>,
}

impl World {
    pub fn new() -> Self {
        let root = tempfile::tempdir().unwrap();
        let state = Arc::new(Mutex::new(initial()));
        let shared = Arc::clone(&state);
        let server = Server::start(move |request| serve(&shared, request));
        let config = "[phase]\nowner = 'PerishLab'\norganization = 'O_lab'\nfield = 'IFSS_phase'\n[phase.options]\nClarify = 'IFSSO_c'\nDeliver = 'IFSSO_d'\nVerify = 'IFSSO_v'\nSettle = 'IFSSO_s'\n";
        std::fs::write(root.path().join("config.toml"), config).unwrap();
        let world = Self {
            root,
            server,
            state,
        };
        decoded(&world.command(&["issue", "bootstrap"]));
        world
    }

    pub fn command(&self, args: &[&str]) -> Output {
        self.attached(&self.server, args)
    }

    pub fn attached(&self, server: &Server, args: &[&str]) -> Output {
        let mut command = spawn::concord(self.root.path());
        server.attach(&mut command);
        command
            .current_dir(self.root.path())
            .args([
                "--home",
                self.root.path().to_str().unwrap(),
                "--config",
                self.root.path().join("config.toml").to_str().unwrap(),
                "--json",
            ])
            .args(args)
            .output()
            .unwrap()
    }

    pub fn phase(&self, operation: &str, input: &Value) -> Output {
        let path = self.root.path().join("input.json");
        std::fs::write(&path, input.to_string()).unwrap();
        let mut args = vec!["issue", "phase", operation];
        match operation {
            "prepare" => args.push("PerishLab/concord#4"),
            "apply" => args.push("--apply"),
            _ => {}
        }
        args.extend(["--input", path.to_str().unwrap()]);
        self.command(&args)
    }

    pub fn observe(&self, mode: &str) -> Value {
        let args = match mode {
            "brief" => vec!["issue", "brief", "PerishLab/concord#4"],
            "preflight" => vec!["issue", "preflight", "existing", "PerishLab/concord#4"],
            "next" => vec!["project", "next", "PerishLab/1"],
            _ => unreachable!(),
        };
        decoded(&self.command(&args))
    }

    pub fn human(&self, args: &[&str]) -> String {
        let mut command = spawn::concord(self.root.path());
        self.server.attach(&mut command);
        let result = command
            .current_dir(self.root.path())
            .args([
                "--home",
                self.root.path().to_str().unwrap(),
                "--config",
                self.root.path().join("config.toml").to_str().unwrap(),
            ])
            .args(args)
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        String::from_utf8(result.stdout).unwrap()
    }

    pub fn change(&self, change: impl FnOnce(&mut Value)) {
        change(&mut self.state.lock().unwrap());
    }

    pub fn state(&self) -> Value {
        self.state.lock().unwrap().clone()
    }

    pub fn writes(&self) -> usize {
        self.state()["writes"].as_array().unwrap().len()
    }

    pub fn unbind(&self) {
        std::fs::write(self.root.path().join("config.toml"), "").unwrap();
    }
}

pub(super) fn page(nodes: Vec<Value>) -> Value {
    json!({"totalCount":nodes.len(),"nodes":nodes,"pageInfo":{"hasNextPage":false,"endCursor":null}})
}

pub(super) fn intent(value: Value) -> Value {
    json!({"reason":"Review the remaining work against the original promise.","value":value})
}

pub(super) fn decoded(output: &Output) -> Value {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

pub(super) fn refused(output: &Output, code: &str) {
    assert!(
        !output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
    let error: Value = serde_json::from_slice(&output.stderr).unwrap();
    assert_eq!(error["error"]["code"], code, "{error}");
}

fn initial() -> Value {
    let empty = page(vec![]);
    let options = json!([
        {"id":"IFSSO_c","name":"Clarify"},{"id":"IFSSO_d","name":"Deliver"},
        {"id":"IFSSO_v","name":"Verify"},{"id":"IFSSO_s","name":"Settle"}
    ]);
    let field = json!({
        "__typename":"IssueFieldSingleSelect","id":"IFSS_phase","fullDatabaseId":"91",
        "name":"Phase","dataType":"SINGLE_SELECT","visibility":"ORG_ONLY","options":options
    });
    let labels = page(vec![json!({"id":"L_release","name":"acceptance:release"})]);
    let values = page(vec![json!({
        "__typename":"IssueFieldTextValue","id":"V_other","value":"Keep","field":{"id":"IFT_other"}
    })]);
    let repo = json!({"id":"R_repo","nameWithOwner":"PerishLab/concord","owner":{"id":"O_lab"}});
    let summary = json!({"total":0,"completed":0,"percentCompleted":0});
    let body = "## Problem\nNeed phase\n## Outcome\nInstalled behavior\n## Scope\nNarrow\n## Acceptance\n- [ ] Installed\n## Non-goals\nNo automation";
    let issue = json!({
        "__typename":"Issue","id":"I_work","number":4,
        "url":"https://github.com/PerishLab/concord/issues/4","title":"Work","body":body,
        "state":"OPEN","updatedAt":"original","viewerCanUpdate":true,
        "issueType":{"id":"IT_feature","name":"Feature"},"repository":repo,"parent":null,
        "subIssues":empty,"subIssuesSummary":summary,"labels":labels,"blockedBy":empty,
        "blocking":empty,"comments":empty,"timelineItems":empty,"issueFieldValues":values
    });
    json!({"field":field,"issue":issue,"owner":"O_lab","writes":[],"reads":[],"mode":"ordinary"})
}
