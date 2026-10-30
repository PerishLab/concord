use crate::spawn;
use serde_json::{Value, json};
use std::path::PathBuf;
use std::process::Output;

pub(super) struct World {
    pub(super) root: tempfile::TempDir,
    provider: PathBuf,
}

impl World {
    pub fn new() -> Self {
        let root = tempfile::tempdir().unwrap();
        let provider = root.path().join("provider");
        plumb::testing::executable(&provider, PROVIDER);
        let empty = page(vec![]);
        let field = json!({"__typename":"IssueFieldSingleSelect","id":"IFSS_phase","fullDatabaseId":"91","name":"Phase","dataType":"SINGLE_SELECT","visibility":"ORG_ONLY","options":[{"id":"IFSSO_c","name":"Clarify"},{"id":"IFSSO_d","name":"Deliver"},{"id":"IFSSO_v","name":"Verify"},{"id":"IFSSO_s","name":"Settle"}]});
        let issue = json!({"__typename":"Issue","id":"I_work","number":4,"url":"https://github.com/PerishLab/concord/issues/4","title":"Work","body":"## Problem\nNeed phase\n## Outcome\nInstalled behavior\n## Scope\nNarrow\n## Acceptance\n- [ ] Installed\n## Non-goals\nNo automation","state":"OPEN","updatedAt":"original","viewerCanUpdate":true,"issueType":{"id":"IT_feature","name":"Feature"},"repository":{"id":"R_repo","nameWithOwner":"PerishLab/concord","owner":{"id":"O_lab"}},"parent":null,"subIssues":empty,"subIssuesSummary":{"total":0,"completed":0,"percentCompleted":0},"labels":page(vec![json!({"id":"L_release","name":"acceptance:release"})]),"blockedBy":empty,"blocking":empty,"comments":empty,"timelineItems":empty,"issueFieldValues":page(vec![json!({"__typename":"IssueFieldTextValue","id":"V_other","value":"Keep","field":{"id":"IFT_other"}})])});
        let state =
            json!({"field":field,"issue":issue,"owner":"O_lab","writes":[],"mode":"ordinary"});
        std::fs::write(root.path().join("state.json"), state.to_string()).unwrap();
        let config = "[phase]\nowner = 'PerishLab'\norganization = 'O_lab'\nfield = 'IFSS_phase'\n[phase.options]\nClarify = 'IFSSO_c'\nDeliver = 'IFSSO_d'\nVerify = 'IFSSO_v'\nSettle = 'IFSSO_s'\n";
        std::fs::write(root.path().join("config.toml"), config).unwrap();
        let world = Self { root, provider };
        let output = world.command(&["issue", "bootstrap"]);
        decoded(&output);
        world
    }

    pub fn command(&self, args: &[&str]) -> Output {
        spawn::concord(self.root.path())
            .current_dir(self.root.path())
            .args([
                "--root",
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
        args.extend([
            "--github-command",
            self.provider.to_str().unwrap(),
            "--input",
            path.to_str().unwrap(),
        ]);
        self.command(&args)
    }

    pub fn observe(&self, mode: &str) -> Value {
        let args = match mode {
            "brief" => vec!["issue", "brief", "PerishLab/concord#4"],
            "preflight" => vec!["issue", "preflight", "existing", "PerishLab/concord#4"],
            "next" => vec!["project", "next", "PerishLab/1"],
            _ => unreachable!(),
        };
        let mut args = args;
        args.extend(["--github-command", self.provider.to_str().unwrap()]);
        decoded(&self.command(&args))
    }

    pub fn human(&self, args: &[&str]) -> String {
        let result = spawn::concord(self.root.path())
            .current_dir(self.root.path())
            .args([
                "--root",
                self.root.path().to_str().unwrap(),
                "--config",
                self.root.path().join("config.toml").to_str().unwrap(),
            ])
            .args(args)
            .args(["--github-command", self.provider.to_str().unwrap()])
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
        let mut state = self.state();
        change(&mut state);
        std::fs::write(self.root.path().join("state.json"), state.to_string()).unwrap();
    }

    pub fn state(&self) -> Value {
        serde_json::from_slice(&std::fs::read(self.root.path().join("state.json")).unwrap())
            .unwrap()
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

const PROVIDER: &str = r##"#!/usr/bin/env python3
import copy,datetime,json,pathlib,sys
p=pathlib.Path(__file__).with_name('state.json');s=json.loads(p.read_text())
def page(nodes):return {'totalCount':len(nodes),'nodes':nodes,'pageInfo':{'hasNextPage':False,'endCursor':None}}
def emit(data):print(json.dumps({'data':data}));sys.exit(0)
def save():p.write_text(json.dumps(s))
args=sys.argv[1:]
if '--input' in args:r=json.load(sys.stdin);q=r['query'];v=r.get('variables',{})
else:q=next(a[6:] for a in args if a.startswith('query='));v={}
kind='schema' if 'issueFields(first:' in q else 'values' if 'issueFieldValues(first:' in q else 'peer' if 'issues(states:' in q else 'root'
s.setdefault('reads',[]).append(kind);save()
if s['mode']=='unavailable' or (s['mode']=='provider' and kind=='root') or (s['mode']=='peer' and kind=='peer'):sys.exit(1)
if q.startswith('mutation'):
    s['writes'].append(r);f=v['input']['issueFields'];assert len(f)==1 and f[0]['fieldId']=='IFSS_phase'
    values=s['issue']['issueFieldValues']['nodes'];values[:]=[x for x in values if x['field']['id']!='IFSS_phase']
    if not f[0].get('delete'):
        o=next(o for o in s['field']['options'] if o['id']==f[0]['singleSelectOptionId'])
        values.append({'__typename':'IssueFieldSingleSelectValue','id':'V_phase','optionId':o['id'],'name':o['name'],'field':{'id':'IFSS_phase'}})
    s['issue']['issueFieldValues']=page(values)
    if s['mode']=='stamp':s['issue']['updatedAt']='phase-only'
    if s['mode']=='foreign':s['issue']['body']='Changed by another actor'
    save()
    if s['mode']=='unknown':sys.exit(1)
    emit({'change':{'clientMutationId':None}})
if 'issueFields(first:' in q:emit({'organization':{'id':s['owner'],'login':'PerishLab','issueFields':page([s['field']])}})
i=copy.deepcopy(s['issue'])
if s['mode']=='drift' and kind=='values':i['id']='I_changed'
if s['mode']=='paging' and kind=='root':i['blocking']['pageInfo']={'hasNextPage':True,'endCursor':None}
if 'issueFieldValues(first:' in q and s['mode']=='minimal':
    for word in ['body','state','updatedAt','viewerCanUpdate','parent','subIssues','labels','blockedBy','blocking','comments','issueType']:assert word not in q
    i={key:i[key] for key in ['id','number','url','repository','issueFieldValues']}
    emit({'repository':{'id':'R_repo','issue':i}})
if 'projectV2(number:' in q or 'on ProjectV2' in q:
    project={'__typename':'ProjectV2','id':'PVT_plan','number':1,'title':'Plan','url':'https://github.com/orgs/PerishLab/projects/1','updatedAt':'original','closed':False,'public':False,'readme':'Boundary','shortDescription':'Goal','viewerCanUpdate':True,'owner':{'__typename':'Organization','id':'O_lab','login':'PerishLab'}}
    today=(datetime.datetime.now(datetime.timezone.utc)+datetime.timedelta(hours=8)).date().isoformat()
    cycle={'__typename':'ProjectV2IterationField','id':'F_cycle','name':'Cycle','dataType':'ITERATION','isIssueField':False,'project':{'id':'PVT_plan'},'updatedAt':'original','configuration':{'duration':7,'startDay':1,'iterations':[{'id':'C_now','title':'Now','startDate':today,'duration':7}],'completedIterations':[]}}
    priority={'__typename':'ProjectV2SingleSelectField','id':'F_priority','name':'Priority','dataType':'SINGLE_SELECT','isIssueField':False,'project':{'id':'PVT_plan'},'updatedAt':'original','options':[{'id':str(n),'name':name} for n,name in enumerate(['Urgent','High','Medium','Low'])]}
    first={'id':'ITEM_first','type':'ISSUE','isArchived':False,'project':{'id':'PVT_plan'},'updatedAt':'original','content':i,'fieldValues':page([{'__typename':'ProjectV2ItemFieldIterationValue','field':{'id':'F_cycle'},'iterationId':'C_now'}])}
    second=copy.deepcopy(first);second['id']='ITEM_second';second['content']['id']='I_second'
    project['fields']=page([priority,cycle]);project['items']=page([first,second])
    if 'projectV2(number:' in q:
        for key in ['fields','items']:project.pop(key)
        emit({'organization':{'projectV2':project}})
    if 'fields(first:' not in q:project.pop('fields')
    if 'items(first:' not in q:project.pop('items')
    emit({'node':project})
if 'node(id:' in q:emit({'node':i})
emit({'repository':{'id':'R_repo','issue':i}})
"##;
