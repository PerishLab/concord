use super::super::{evaluate, world};
use concord_core::acceptance::{Judgment, Marker, Reference, Target, Verification, digest};
use serde_json::{Value, json};
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Output, Stdio};

pub(super) struct World {
    root: tempfile::TempDir,
    provider: PathBuf,
}

impl World {
    pub fn new() -> Self {
        let root = tempfile::tempdir().unwrap();
        let provider = root.path().join("provider");
        let held = evaluate::World::new();
        let data = json!({"page":held.page,"fresh":held.fresh,"relations":held.relations});
        std::fs::write(root.path().join("data.json"), data.to_string()).unwrap();
        std::fs::write(
            root.path().join("state.json"),
            json!({"mode":"ok","closed":false,"attempts":0,"heads":0,"calls":0}).to_string(),
        )
        .unwrap();
        plumb::testing::executable(&provider, format!("#!/usr/bin/env python3\n{SCRIPT}"));
        Self { root, provider }
    }

    pub fn release(&self) {
        let path = self.root.path().join("data.json");
        let mut data: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        let declaration = Marker::Declaration {
            target: Target::Release,
            promise: "Source and complete stable".into(),
        }
        .render()
        .unwrap();
        let body = data["page"]["data"]["repository"]["issue"]["body"]
            .as_str()
            .unwrap();
        let closure = Marker::Closure {
            target: Target::Release,
            declaration: Reference {
                node: "IC_decl".into(),
                digest: digest(&declaration),
            },
            judgment: Judgment::Satisfied,
            verification: Verification::Manual,
            review: digest(body),
            evidence: vec!["reviewed source".into()],
            remaining: vec![],
            release: Some(concord_core::acceptance::Release {
                marker: "v1.0.0".into(),
                distribution:
                    "https://releases.example.org/v1/releases/stable/v1.0.0/distribution.json"
                        .into(),
                inclusion: vec!["manual inclusion".into()],
            }),
        }
        .render()
        .unwrap();
        let issue = &mut data["page"]["data"]["repository"]["issue"];
        issue["labels"] = world::connection(vec![json!({"name":"acceptance:release"})], None, 1);
        issue["comments"] = world::connection(
            vec![
                world::entry("IC_decl", &declaration, 1),
                world::entry("IC_close", &closure, 2),
            ],
            None,
            2,
        );
        data["fresh"]["data"]["declaration"]["body"] = declaration.into();
        data["fresh"]["data"]["closure"]["body"] = closure.into();
        std::fs::write(path, data.to_string()).unwrap();
    }

    pub fn mode(&self, mode: &str) {
        let mut value = self.state();
        value["mode"] = mode.into();
        std::fs::write(self.root.path().join("state.json"), value.to_string()).unwrap();
    }

    pub fn state(&self) -> Value {
        serde_json::from_slice(&std::fs::read(self.root.path().join("state.json")).unwrap())
            .unwrap()
    }

    pub fn command(&self, operation: &str) -> Command {
        let mut command = crate::spawn::concord(self.root.path());
        command
            .args([
                "--space",
                "/deliberately-unavailable-estate",
                "issue",
                "close",
                operation,
                "PerishLab/concord#83",
                "--github-command",
            ])
            .arg(&self.provider);
        command
    }

    pub fn run(&self, operation: &str, input: Option<&Value>) -> Output {
        let mut command = self.command(operation);
        command.arg("--json");
        if operation == "apply" {
            command.arg("--apply");
        }
        let output = output(command, input);
        assert!(!self.root.path().join(".concord").exists());
        assert!(!self.root.path().join(".issues").exists());
        output
    }

    pub fn prepare(&self) -> Value {
        world::decoded(&self.run("prepare", None))
    }
}

pub(super) fn output(mut command: Command, input: Option<&Value>) -> Output {
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command.spawn().unwrap();
    if let Some(input) = input {
        child
            .stdin
            .take()
            .unwrap()
            .write_all(input.to_string().as_bytes())
            .unwrap();
    } else {
        drop(child.stdin.take());
    }
    child.wait_with_output().unwrap()
}

const SCRIPT: &str = r#"
import json, pathlib, sys
root = pathlib.Path(__file__).parent
state = json.loads((root / 'state.json').read_text())
data = json.loads((root / 'data.json').read_text())
mode = state['mode']
request = json.load(sys.stdin)
query = request['query']
assert '--hostname' in sys.argv and sys.argv[sys.argv.index('--hostname')+1] == 'github.com'

def save():
    (root / 'state.json').write_text(json.dumps(state))

state['calls'] += 1
save()

def issue(value):
    if state['closed']:
        value['state'] = 'CLOSED'
    if state['closed'] or mode in ('stamp', 'reopen'):
        value['updated'] = '2026-10-02T00:00:00Z'
    if mode == 'body' or (mode == 'body-after' and state['closed']):
        value['body'] += '\nChanged promised result\n'
    return value

if query.startswith('mutation ConcordIssueClose'):
    state['attempts'] += 1
    save()
    if mode == 'fail-before':
        sys.exit(8)
    if mode == 'forbidden':
        print(json.dumps({'data':None,'errors':[{'message':'forbidden'}]}))
        sys.exit(0)
    assert request['variables'] == {'id':'I_issue'}
    assert 'stateReason:COMPLETED' in query
    state['closed'] = True
    save()
    if mode == 'lost':
        sys.exit(9)
    value = {'id':'wrong' if mode == 'ack' else 'I_issue','state':'CLOSED','reason':'COMPLETED','repository':{'id':'R_concord'}}
    print(json.dumps({'data':{'closeIssue':{'issue':value}}}))
elif query.startswith('mutation'):
    sys.exit(91)
elif query.startswith('query ConcordIssueClose'):
    state['heads'] += 1
    save()
    if mode == 'readback' and state['closed']:
        sys.exit(7)
    if mode == 'partial':
        print(json.dumps({'data':None,'errors':[{'message':'partial'}]}))
        sys.exit(0)
    value = issue(data['page']['data']['repository']['issue'])
    value['writable'] = mode != 'permission'
    value['reason'] = 'COMPLETED' if state['closed'] else None
    if mode == 'reason':
        value['reason'] = 'COMPLETED'
    if mode in ('auto','unknown'):
        value['kind']['name'] = 'Auto' if mode == 'auto' else 'Ghost'
    if mode == 'head-drift' and state['heads'] > 1:
        value['updated'] = '2026-10-03T00:00:00Z'
    if mode == 'node':
        value['id'] = 'I_foreign'
    if mode == 'coordinate':
        value['number'] = 84
    repository = 'R_foreign' if mode == 'repository' else 'R_concord'
    print(json.dumps({'data':{'repository':{'id':repository,'nameWithOwner':'PerishLab/concord','issue':value}}}))
else:
    if 'children:subIssues' in query:
        reply = data['relations']
        if mode in ('child','blocked') or (mode == 'after-unmet' and state['closed']):
            key = 'children' if mode == 'child' else 'blockers'
            reply['data']['repository']['issue'][key] = {'total':1,'nodes':[{'id':'I_blocker','state':'OPEN'}],'page':{'next':False,'cursor':None}}
    elif 'events:timelineItems' in query:
        reply = data['fresh']
        if mode == 'unordered' or (mode == 'after-unknown' and state['closed']):
            reply['data']['repository']['issue']['edited'] = '2026-09-30T00:00:00Z'
        if mode == 'reopen':
            reply['data']['repository']['issue']['events'] = {'total':1,'nodes':[{'id':'RE_new','created':'2026-10-01T00:00:00Z'}],'page':{'next':False,'cursor':None}}
        if mode == 'closure':
            reply['data']['closure']['body'] += '\nNew attestation prose'
        if mode == 'declaration':
            reply['data']['declaration']['body'] += '\nNew promise prose'
    elif 'object(expression:' in query:
        print(json.dumps({'data':{'repository':{'id':'R_concord','object':None}}}))
        sys.exit(0)
    else:
        reply = data['page']
        value = reply['data']['repository']['issue']
        if mode == 'closure':
            value['comments']['nodes'][1]['body'] += '\nNew attestation prose'
        if mode == 'declaration':
            value['comments']['nodes'][0]['body'] += '\nNew promise prose'
        if mode == 'needs':
            value['labels']['nodes'].append({'name':'needs:revalidation'})
            value['labels']['total'] += 1
        if mode == 'legacy':
            value['comments'] = {'total':0,'nodes':[],'page':{'next':False,'cursor':None}}
            value['labels'] = {'total':0,'nodes':[],'page':{'next':False,'cursor':None}}
    if 'repository' in reply['data']:
        issue(reply['data']['repository']['issue'])
    print(json.dumps(reply))
"#;
