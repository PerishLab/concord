use super::{Client, observe};
use serde_json::{Value, json};
use std::path::PathBuf;

pub(super) struct World {
    root: tempfile::TempDir,
    command: PathBuf,
    timeout: u64,
}

impl World {
    pub(super) fn new(mode: &str) -> Self {
        let root = tempfile::tempdir().unwrap();
        let command = root.path().join("provider");
        plumb::testing::executable(
            &command,
            format!("#!/usr/bin/env python3\nMODE = {mode:?}\n{SCRIPT}"),
        );
        Self {
            root,
            command,
            timeout: if mode == "timeout" { 1 } else { 5 },
        }
    }

    pub(super) async fn run(&self, count: usize, pages: u16) -> (concord_core::Result<()>, Value) {
        let client = Client {
            command: &self.command,
            timeout: self.timeout,
        };
        let items = (1..=count).map(item).collect::<Vec<_>>();
        let field = json!({"id":"F_attached", "authority":"issue", "dataType":"SINGLE_SELECT", "issueField":{"fullDatabaseId":"42"}});
        let mut collections = json!({"fields":{"nodes":[field]},"items":{"nodes":items}});
        let result = observe(&client, &mut collections, pages).await;
        (result, collections)
    }

    pub(super) fn state(&self) -> Value {
        serde_json::from_slice(&std::fs::read(self.root.path().join("state")).unwrap()).unwrap()
    }

    pub(super) fn client(&self) -> Client<'_> {
        Client {
            command: &self.command,
            timeout: self.timeout,
        }
    }

    #[cfg(target_os = "linux")]
    pub(super) async fn quiet(&self) {
        for _ in 0..100 {
            if !self.alive() {
                return;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
        assert!(!self.alive(), "provider children must be killed and reaped");
    }

    #[cfg(target_os = "linux")]
    fn alive(&self) -> bool {
        std::fs::read_dir(self.root.path()).unwrap().any(|entry| {
            let name = entry.unwrap().file_name().to_string_lossy().into_owned();
            name.strip_prefix("pid-")
                .is_some_and(|pid| std::path::Path::new(&format!("/proc/{pid}")).exists())
        })
    }
}

fn item(number: usize) -> Value {
    let kind = match number % 3 {
        0 => "Auto",
        1 => "Feature",
        _ => "Unknown",
    };
    json!({"content":{
        "__typename":"Issue", "id":format!("I_{number}"), "number":number,
        "updatedAt":"unchanged", "repository":{"id":"R_repo", "nameWithOwner":"PerishLab/concord"},
        "issueType":{"name":kind}
    }})
}

const SCRIPT: &str = r#"
import fcntl, json, os, pathlib, sys, time
root = pathlib.Path(__file__).parent

def state(change=None):
    with (root / 'lock').open('a') as lock:
        fcntl.flock(lock, fcntl.LOCK_EX)
        path = root / 'state'
        value = json.loads(path.read_text()) if path.exists() else {'active':0, 'peak':0, 'open':False, 'events':[], 'confirmed':[]}
        if change:
            change(value)
            path.write_text(json.dumps(value))
        return value

def record(stage, number):
    def change(value):
        value['events'].append([stage, number])
        if stage == 'start':
            value['active'] += 1
            value['peak'] = max(value['peak'], value['active'])
            value['open'] = value['open'] or value['active'] == 4
        if stage in ('confirmed', 'refused'):
            value['active'] -= 1
        if stage == 'confirmed':
            value['confirmed'].append(number)
    state(change)

def until(predicate):
    end = time.monotonic() + 2
    while not predicate(state()):
        if time.monotonic() >= end:
            sys.exit(7)
        time.sleep(0.005)

def field(number):
    return {'issue_field_id':number, 'data_type':'single_select', 'value':number}

if sys.argv[1:4] == ['api', '--method', 'GET']:
    route = sys.argv[4]
    number = int(route.split('/issues/')[1].split('/')[0])
    page = int(route.split('page=')[-1])
    if page == 1:
        record('start', number)
    record('page', number)
    if MODE == 'parallel' and number <= 4:
        until(lambda value: value['open'])
    if MODE == 'refused' and number == 1:
        record('refused', number)
        sys.exit(8)
    if MODE == 'timeout':
        (root / f'pid-{os.getpid()}').touch()
        time.sleep(3)
    if MODE == 'partial':
        print('{}')
    elif MODE == 'repeat':
        print(json.dumps([field(42), field(42)]))
    elif MODE == 'type':
        print(json.dumps([{'issue_field_id':42, 'data_type':'text'}]))
    elif MODE == 'pages':
        print(json.dumps([field(i) for i in range(1, 101)] if page == 1 else []))
    elif MODE == 'empty':
        print('[]')
    else:
        value = field(42)
        value['value'] = number
        print(json.dumps([value]))
elif sys.argv[1:] == ['api', 'graphql', '--input', '-']:
    request = json.load(sys.stdin)
    number = int(request['variables']['id'].split('_')[-1])
    if MODE == 'parallel' and number <= 4:
        until(lambda value: all(i in value['confirmed'] for i in range(number + 1, 5)))
    record('confirmed', number)
    value = {'id':f'I_{number}', 'updatedAt':'unchanged', 'repository':{'id':'R_repo'}}
    if MODE == 'node':
        value['id'] = 'I_foreign'
    if MODE == 'repository':
        value['repository']['id'] = 'R_foreign'
    if MODE == 'stamp':
        value['updatedAt'] = 'changed'
    reply = {'data':{'node':value}}
    if MODE == 'graphql':
        reply['errors'] = [{'message':'partial response'}]
    print(json.dumps(reply))
else:
    sys.exit(9)
"#;
