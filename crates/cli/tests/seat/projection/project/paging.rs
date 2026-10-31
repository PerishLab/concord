use super::super::super::spawn;
use serde_json::{Value, json};
use std::process::Output;

#[test]
fn pages() {
    let mut selected = Vec::new();
    for (size, expected) in [(None, 5), (Some(20), 11), (Some(100), 3)] {
        let fixture = Fixture::new("ordinary", 208);
        let result = fixture.success("next", size, 20);
        let calls = fixture.calls("items");
        assert_eq!(calls.len(), expected, "page size {size:?}");
        assert!(calls.iter().all(|call| call["size"] == size.unwrap_or(50)));
        assert_eq!(
            fixture.calls("fields")[0]["size"],
            size.unwrap_or(50).min(20)
        );
        assert_eq!(fixture.calls("confirm").len(), 2);
        assert_eq!(fixture.calls("final").len(), 1);
        let next = &result["next"];
        assert_eq!(next["item"], "ITEM_139");
        assert_eq!(next["issue"]["id"], "I_139");
        assert_eq!(next["priority"]["name"], "Low");
        selected.push(json!({"issue":next["issue"],"item":next["item"],"priority":next["priority"],"cycle":next["cycle"],"fields":next["fields"],"reason":next["reason"]}));
    }
    assert!(selected.windows(2).all(|pair| pair[0] == pair[1]));
}

#[test]
fn bounds() {
    let fixture = Fixture::new("ordinary", 7);
    let one = fixture.success("next", Some(1), 7);
    assert_eq!(fixture.calls("items").len(), 7);
    assert_eq!(one["next"]["item"], "ITEM_4");
    let fixture = Fixture::new("ordinary", 7);
    assert_eq!(
        fixture.success("next", Some(100), 7)["next"]["item"],
        one["next"]["item"]
    );
    let fixture = Fixture::new("ordinary", 208);
    fixture.refused(Some(50), 4, "concord.project.incomplete");
    assert_eq!(fixture.calls("items").len(), 4);
    assert!(fixture.calls("confirm").is_empty());
}

#[test]
fn failures() {
    for (mode, code) in [
        ("failure", "concord.project.provider"),
        ("truncated", "concord.project.changed"),
        ("duplicate", "concord.project.changed"),
        ("cursor", "concord.project.schema"),
        ("metadata", "concord.project.changed"),
        ("selected", "concord.project.changed"),
        ("later", "concord.project.changed"),
        ("final", "concord.project.changed"),
    ] {
        let fixture = Fixture::new(mode, 208);
        fixture.refused(Some(50), 20, code);
        assert!(fixture.calls("items").len() <= 5, "{mode}");
    }
}

#[test]
fn complete() {
    let fixture = Fixture::new("ordinary", 208);
    let result = fixture.success("show", Some(100), 20);
    assert_eq!(result["project"]["complete"], true);
    assert_eq!(
        result["project"]["collections"]["items"]["nodes"]
            .as_array()
            .unwrap()
            .len(),
        208
    );
    assert_eq!(fixture.calls("items").len(), 11);
    assert!(fixture.calls("items").iter().all(|call| call["size"] == 20));
    assert_eq!(fixture.calls("fields")[0]["size"], 100);
}

pub(super) struct Fixture {
    root: tempfile::TempDir,
}

impl Fixture {
    pub fn new(mode: &str, count: usize) -> Self {
        let root = tempfile::tempdir().unwrap();
        std::fs::write(
            root.path().join("state.json"),
            json!({"mode":mode,"count":count}).to_string(),
        )
        .unwrap();
        plumb::testing::executable(&root.path().join("provider"), PROVIDER);
        Self { root }
    }

    fn run(&self, operation: &str, size: Option<u16>, pages: u16) -> Output {
        let mut command = spawn::concord(self.root.path());
        command
            .args([
                "--space",
                "/deliberately-unavailable-estate",
                "project",
                operation,
                "PerishLab/1",
                "--json",
                "--github-command",
            ])
            .arg(self.root.path().join("provider"))
            .arg("--max-pages")
            .arg(pages.to_string());
        if let Some(size) = size {
            command.arg("--page-size").arg(size.to_string());
        }
        command.output().unwrap()
    }

    pub fn success(&self, operation: &str, size: Option<u16>, pages: u16) -> Value {
        let output = self.run(operation, size, pages);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(!self.root.path().join(".concord").exists());
        serde_json::from_slice(&output.stdout).unwrap()
    }

    pub fn refused(&self, size: Option<u16>, pages: u16, code: &str) {
        let output = self.run("next", size, pages);
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
        let error: Value = serde_json::from_slice(&output.stderr).unwrap();
        assert_eq!(error["error"]["code"], code);
    }

    pub fn calls(&self, kind: &str) -> Vec<Value> {
        std::fs::read_to_string(self.root.path().join("calls.jsonl"))
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str::<Value>(line).unwrap())
            .filter(|call| call["kind"] == kind)
            .collect()
    }
}

const PROVIDER: &str = r#"#!/usr/bin/env python3
import datetime,json,sys
from pathlib import Path
root=Path(__file__).parent
state=json.loads((root/'state.json').read_text());mode=state['mode'];count=state['count']
assert sys.argv[1:]==['api','graphql','--input','-']
request=json.load(sys.stdin);q=request['query'];v=request['variables'];assert q.startswith('query')
meta={'__typename':'ProjectV2','id':'PVT_plan','number':1,'title':'Planning','url':'https://github.com/orgs/PerishLab/projects/1','updatedAt':'original','closed':False,'public':False,'readme':'Boundary','shortDescription':'Goal','viewerCanUpdate':True,'owner':{'__typename':'Organization','id':'O_lab','login':'PerishLab'}}
def connection(nodes):return {'totalCount':len(nodes),'nodes':nodes,'pageInfo':{'hasNextPage':False,'endCursor':None}}
day=(datetime.datetime.now(datetime.timezone.utc)+datetime.timedelta(hours=8)).date().isoformat()
period={'id':'current','title':'Current','startDate':day,'duration':7}
common={'isIssueField':False,'updatedAt':'original','project':{'id':'PVT_plan'}}
fields=[dict(common,__typename='ProjectV2IterationField',id='F_cycle',name='Cycle',dataType='ITERATION',configuration={'duration':7,'startDay':1,'iterations':[period],'completedIterations':[]}),dict(common,__typename='ProjectV2SingleSelectField',id='F_priority',name='Priority',dataType='SINGLE_SELECT',options=[{'id':name,'name':name} for name in ['Urgent','High','Medium','Low']])]
chosen=139 if count>100 else 4
def issue(i):
 empty=connection([])
 return {'__typename':'Issue','id':f'I_{i}','number':i+1,'title':f'Work {i}','url':f'https://github.com/PerishLab/concord/issues/{i+1}','state':'OPEN','body':'Research fixture','updatedAt':'original','issueType':{'id':'IT_feature','name':'Feature'},'repository':{'id':'R_repo','nameWithOwner':'PerishLab/concord','owner':{'id':'O_lab'}},'parent':None,'blockedBy':connection([{'id':'I_block','url':'https://github.com/PerishLab/concord/issues/999','state':'OPEN'}]) if i==chosen-1 else empty,'blocking':empty,'subIssues':empty,'labels':connection([{'id':'L_needs','name':'needs:revalidation'}]) if i==chosen-2 else empty,'comments':empty}
def item(i):
 values=[{'__typename':'ProjectV2ItemFieldIterationValue','iterationId':'current' if i>=chosen-2 else 'backlog','title':'Current','startDate':day,'duration':7,'field':{'id':'F_cycle'}},{'__typename':'ProjectV2ItemFieldSingleSelectValue','optionId':'Low' if i==chosen else 'Urgent','name':'Low' if i==chosen else 'Urgent','field':{'id':'F_priority'}}]
 return {'id':f'ITEM_{i}','type':'ISSUE','isArchived':False,'updatedAt':'original','project':{'id':'PVT_plan'},'content':issue(i),'fieldValues':connection(values)}
kind='root' if 'organization(login:' in q else next((name for name in ['fields','items','views','workflows','repositories','statusUpdates'] if name+'(first:' in q),None)
if kind is None:kind='confirm' if '... on Issue{' in q else 'final'
log=root/'calls.jsonl';previous=[json.loads(line) for line in log.read_text().splitlines()] if log.exists() else []
with log.open('a') as output:output.write(json.dumps({'kind':kind,'size':v.get('size'),'after':v.get('after')})+'\n')
if kind=='root':print(json.dumps({'data':{'organization':{'projectV2':meta}}}));sys.exit(0)
if kind=='confirm':
 value=issue(chosen)
 if mode=='selected' or (mode=='later' and any(c['kind']=='confirm' for c in previous)):value['updatedAt']='changed'
 print(json.dumps({'data':{'node':value}}));sys.exit(0)
if kind in ['fields','items','views','workflows','repositories','statusUpdates']:
 start=int(v.get('after') or 0);size=v['size'];nodes=fields if kind=='fields' else [item(i) for i in range(count)] if kind=='items' else []
 page=nodes[start:start+size];more=start+size<len(nodes);cursor=str(start+size) if more else None;total=len(nodes)
 if kind=='items' and start:
  if mode=='failure':sys.exit(7)
  if mode=='truncated':more=False
  if mode=='duplicate':page[0]['id']='ITEM_0'
  if mode=='cursor':cursor=v['after']
  if mode=='metadata':meta['updatedAt']='changed'
 meta[kind]={'totalCount':total,'nodes':page,'pageInfo':{'hasNextPage':more,'endCursor':cursor}}
if kind=='final' and mode=='final':meta['updatedAt']='changed'
print(json.dumps({'data':{'node':meta}}))
"#;
