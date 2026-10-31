use super::super::spawn;
use super::planning;
use serde_json::{Value, json};
use std::path::PathBuf;
use std::process::Output;

pub(super) struct Fixture {
    root: tempfile::TempDir,
    provider: PathBuf,
}

impl Fixture {
    pub fn new(mode: &str) -> Self {
        let root = tempfile::tempdir().unwrap();
        let provider = root.path().join("provider");
        plumb::testing::executable(&provider, PROVIDER);
        let project = planning::meta();
        let initial = json!({"project":project,"projects":[project],"fields":[field("F_local","Planning",false),field("F_issue","Priority",true)],"items":[],"views":[],"workflows":[],"repositories":[],"statusUpdates":[],"issues":[issue("I_one"),issue("I_two")],"mode":mode,"writes":[],"clock":0});
        std::fs::write(root.path().join("state.json"), initial.to_string()).unwrap();
        Self { root, provider }
    }

    pub fn run(&self, operation: &str, input: &Value) -> Output {
        self.target(operation, "PerishLab/1", input)
    }

    pub fn target(&self, operation: &str, target: &str, input: &Value) -> Output {
        let path = self.root.path().join("input.json");
        std::fs::write(&path, input.to_string()).unwrap();
        let mut command = spawn::concord(self.root.path());
        command.current_dir(self.root.path()).args([
            "--space",
            "/unavailable-estate",
            "--json",
            "project",
            operation,
        ]);
        command.args(["--github-command", self.provider.to_str().unwrap()]);
        let input = ["--input", path.to_str().unwrap()];
        match operation {
            "prepare" => {
                command.arg(target).args(input);
            }
            "list" => {
                command.arg(target);
            }
            "apply" => {
                command.arg("--apply").args(input);
            }
            _ => {
                command.args(input);
            }
        }
        command.output().unwrap()
    }

    pub fn state(&self) -> Value {
        serde_json::from_slice(&std::fs::read(self.root.path().join("state.json")).unwrap())
            .unwrap()
    }

    pub fn change(&self, mutate: impl FnOnce(&mut Value)) {
        let mut state = self.state();
        mutate(&mut state);
        std::fs::write(self.root.path().join("state.json"), state.to_string()).unwrap();
    }
}

fn issue(id: &str) -> Value {
    json!({"__typename":"Issue","id":id,"number":4,"url":"https://github.com/PerishLab/concord/issues/4","state":"OPEN","updatedAt":"issue-original","issueType":{"id":"IT_task","name":"Task"},"repository":{"id":"R_repo","owner":{"id":"O_lab"}}})
}

fn field(id: &str, name: &str, attached: bool) -> Value {
    let mut value = json!({"__typename":"ProjectV2SingleSelectField","id":id,"name":name,"dataType":"SINGLE_SELECT","isIssueField":attached,"updatedAt":"field-original","project":{"id":"PVT_plan"},"options":[{"id":"old","name":"Candidate","color":"GRAY","description":""}]});
    if attached {
        value["issueField"] = json!({"id":"IF_priority","fullDatabaseId":"42","name":name,"dataType":"SINGLE_SELECT"});
    }
    value
}

pub(super) fn intent(action: Value) -> Value {
    json!({"reason":"Review the domain candidate pool inside the entrusted planning boundary.","action":action})
}

pub(super) fn decoded(output: &Output) -> Value {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

pub(super) fn refused(output: &Output, code: &str) -> Value {
    assert!(
        !output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
    let error: Value = serde_json::from_slice(&output.stderr).unwrap();
    assert_eq!(error["error"]["code"], code, "{error}");
    error
}

const PROVIDER: &str = r##"#!/usr/bin/env python3
import copy,json,pathlib,re,sys
path=pathlib.Path(__file__).with_name('state.json')
s=json.loads(path.read_text())
def save(): path.write_text(json.dumps(s))
def emit(value): print(json.dumps(value));sys.exit(0)
def fail(): print('refused',file=sys.stderr);sys.exit(1)
def page(nodes): return {'nodes':nodes,'totalCount':len(nodes),'pageInfo':{'hasNextPage':False,'endCursor':None}}
def stamp():
    s['clock']+=1
    s['project']['updatedAt']='stamp-'+str(s['clock'])
def projected():
    value=copy.deepcopy(s['project'])
    if s.get('projection'):
        for key in ['title','shortDescription','readme','public']:value[key]=s['previous'][key]
    return value
def observed(name):
    nodes=copy.deepcopy(s[name])
    if name!='items' or not s.get('hidden') or s['mode']=='ordinary':return nodes
    s['observations']=s.get('observations',0)+1
    if s['mode']=='reader-failure':save();fail()
    if s.get('delay',0)>0:
        if s['mode']=='delayed-value':
            for item in nodes:
                if item['id']==s['hidden']:item['fieldValues']=s['previous']
        else:nodes=[item for item in nodes if item['content']['id']!=s['hidden']]
        s['delay']-=1
        if s['mode']=='pending-context':s['statusUpdates'].append({'id':'U_foreign','body':'Human changes the investment boundary.','createdAt':'new','updatedAt':'new'})
    save();return nodes
def fields():
    url='https://api.github.com/orgs/PerishLab/projectsV2/'+str(s['project']['number'])
    nodes=[{'id':{'F_local':100,'F_issue':101,'F_new':102}.get(f['id'],200+i),'node_id':f['id'],'name':f['name'],'data_type':f['dataType'].lower(),'project_url':url} for i,f in enumerate(s['fields'])]
    nodes.append({'id':199,'node_id':'F_type','name':'Type','data_type':'issue_type','project_url':url})
    return nodes
def view(payload):
    mapping={f['id']:f['node_id'] for f in fields()}
    refs=lambda ids:page([{'id':mapping[v]} for v in ids])
    visible=refs(payload['visible_fields']);visible.pop('totalCount')
    value={'id':'V_'+str(len(s['views'])),'project':{'id':s['project']['id']},'number':len(s['views'])+1,'fullDatabaseId':'501','name':payload['name'],'layout':payload['layout'].upper()+'_LAYOUT','filter':payload['filter'],'updatedAt':'view-original','configuration':{'visibleFields':visible},'groupByFields':refs(payload['group_by']),'verticalGroupByFields':refs(payload['vertical_group_by']),'sortByFields':page([{'direction':direction.upper(),'field':{'id':mapping[identity]}} for identity,direction in payload['sort_by']])}
    s['views'].append(value);return value
args=sys.argv[1:]
s['calls']=s.get('calls',0)+1;save()
if args[:3]==['api','--method','GET']:
    if '--include' not in args:fail()
    print('HTTP/2.0 200 OK\r\nContent-Type: application/json\r\n\r\n'+json.dumps(fields()),end='');sys.exit(0)
request=json.load(sys.stdin)
if args[:3]==['api','--method','POST']:
    value=view(request);s['writes'].append('rest:view');stamp();save();emit({'value':{'node_id':value['id']}})
if args!=['api','graphql','--input','-']:fail()
q=request['query'];v=request.get('variables',{})
if q.startswith('mutation'):
    name=re.search(r'change:(\w+)',q).group(1);p=v['input'];s['writes'].append(name)
    if name=='createProjectV2StatusUpdate':
        if s['mode']=='record-failure' and 'confirmed-native-state' in p['body']:save();fail()
        s['statusUpdates'].append({'id':'U_'+str(len(s['statusUpdates'])),'body':p['body'],'createdAt':'original','updatedAt':'original'})
        if s['mode']=='request-context':s['statusUpdates'].append({'id':'U_foreign','body':'Human changes the investment boundary.','createdAt':'new','updatedAt':'new'})
    elif name=='addProjectV2ItemById':
        if s['mode']=='partial' and p['contentId']=='I_two':save();fail()
        content=next(i for i in s['issues'] if i['id']==p['contentId'])
        s['items'].append({'id':'ITEM_'+content['id'],'type':'ISSUE','isArchived':False,'updatedAt':'item-original','project':{'id':s['project']['id']},'content':copy.deepcopy(content)})
        if s['mode'].startswith('snapshot'):s['snapshot']=999 if s['mode']=='snapshot-persistent' else 1
        if s['mode'] in ['delayed','invisible','pending-context','reader-failure']:s.update(hidden=content['id'],delay=999 if s['mode']=='invisible' else 1)
        if s['mode']=='changed-type':s['issues'][1]['issueType']['name']='Auto'
        if s['mode']=='context':s['statusUpdates'].append({'id':'U_foreign','body':'Human changes the investment boundary.','createdAt':'new','updatedAt':'new'})
        if s['mode']=='lost':stamp();save();fail()
    elif name=='deleteProjectV2Item':s['items']=[i for i in s['items'] if i['id']!=p['itemId']]
    elif name=='updateProjectV2':
        if s['mode']=='delayed-project':s.update(previous=copy.deepcopy(s['project']),delay=1)
        for k,value in p.items():
            if k!='projectId':s['project'][k]=value
    elif name in ['createProjectV2Field','updateProjectV2Field']:
        if name=='createProjectV2Field':
            value={'__typename':'ProjectV2SingleSelectField','id':'F_new','name':p['name'],'dataType':p['dataType'],'isIssueField':False,'updatedAt':'field-original','project':{'id':s['project']['id']},'options':[]}
            s['fields'].append(value)
        else:
            value=next(f for f in s['fields'] if f['id']==p['fieldId']);value['name']=p['name']
        if 'singleSelectOptions' in p:
            value['options']=p['singleSelectOptions']
            for index,option in enumerate(value['options']):
                if option.get('id') is None:option['id']='choice-'+str(index)
        if 'iterationConfiguration' in p:
            value.update(__typename='ProjectV2IterationField',dataType='ITERATION');periods=copy.deepcopy(p['iterationConfiguration']['iterations'])
            for index,period in enumerate(periods):
                if period.get('id') is None:period['id']='period-new-'+str(index)
            value['configuration']={'duration':p['iterationConfiguration']['duration'],'startDay':3,'iterations':[x for x in periods if x['startDate']>='2026-10-21'],'completedIterations':[x for x in periods if x['startDate']<'2026-10-21']}
    elif name in ['updateProjectV2ItemFieldValue','clearProjectV2ItemFieldValue']:
        item=next(i for i in s['items'] if i['id']==p['itemId']);nodes=[v for v in item['fieldValues']['nodes'] if v['field']['id']!=p['fieldId']]
        if s['mode']=='delayed-value':s.update(hidden=item['id'],delay=1,previous=copy.deepcopy(item['fieldValues']))
        if 'value' in p:
            data=p['value'];value={'field':{'id':p['fieldId']}}
            if 'singleSelectOptionId' in data:value.update(__typename='ProjectV2ItemFieldSingleSelectValue',optionId=data['singleSelectOptionId'])
            else:value.update(__typename='ProjectV2ItemFieldIterationValue',iterationId=data['iterationId'])
            nodes.append(value)
        item['fieldValues']=page(nodes)
    elif name=='updateProjectV2ItemPosition':
        item=next(i for i in s['items'] if i['id']==p['itemId']);s['items'].remove(item)
        index=next((i+1 for i,x in enumerate(s['items']) if x['id']==p['afterId']),0)
        s['items'].insert(index,item)
    elif name=='createProjectV2IssueField':
        origin=s['origin'];assert p=={'projectId':s['project']['id'],'issueFieldId':origin['id']}
        s['fields'].append({'__typename':'ProjectV2SingleSelectField','id':'F_attached','name':origin['name'],'dataType':'SINGLE_SELECT','isIssueField':True,'updatedAt':'attached','project':{'id':s['project']['id']},'issueField':origin,'options':origin['options']})
        if s['mode']=='attachment-unknown':stamp();save();fail()
    elif name=='deleteProjectV2Field':s['fields']=[f for f in s['fields'] if f['id']!=p['fieldId']]
    elif name=='deleteProjectV2Workflow':s['workflows']=[w for w in s['workflows'] if w['id']!=p['workflowId']]
    elif name=='deleteProjectV2View':s['views']=[w for w in s['views'] if w['id']!=p['viewId']]
    elif name=='updateProjectV2View':
        value=next(w for w in s['views'] if w['id']==p['viewId'])
        for key in ['name','filter','layout']:
            if key in p:value[key]=p[key]
        if 'configuration' in p:
            value['configuration']['visibleFields']={'nodes':[{'id':i} for i in p['configuration']['visibleFieldIds']],'pageInfo':{'hasNextPage':False,'endCursor':None}}
    elif name=='createProjectV2':
        value=copy.deepcopy(s['project']);value.update(id='PVT_new',number=2,title=p['title'],url='https://github.com/orgs/PerishLab/projects/2')
        s['project']=value;s['projects'].append(copy.deepcopy(value));s['fields']=[];s['views']=[];s['items']=[];s['statusUpdates']=[]
        s['workflows']=[{'id':'WF_default','name':'Default automation','enabled':True,'updatedAt':'original','project':{'id':'PVT_new'}}]
    elif name in ['linkProjectV2ToRepository','unlinkProjectV2FromRepository']:
        s['repositories']=([{'id':'R_repo','nameWithOwner':'PerishLab/concord','url':'https://github.com/PerishLab/concord','owner':{'id':'O_lab'}}] if name.startswith('link') else [])
    else:fail()
    stamp();save();emit({'data':{'change':{'clientMutationId':None}}})
if 'issueFields(first:' in q:emit({'data':{'organization':{'id':s.get('origin_owner','O_lab'),'issueFields':page([s['origin']])}}})
if 'projectsV2(first:' in q:
    projects=[s['project'] if x['id']==s['project']['id'] else x for x in s['projects']]
    emit({'data':{'organization':{'id':'O_lab','login':'PerishLab','url':'https://github.com/PerishLab','projectsV2':page(projects)}}})
if 'organization(login:' in q:
    s['starts']=s.get('starts',0)+1
    if s['starts']==2:
        if s['mode']=='prepare-project':s['project']['title']='DIAGNOSTIC_SECRET'
        if s['mode']=='prepare-items':s['items'].append({'id':'ITEM_DIAGNOSTIC_SECRET','type':'ISSUE','isArchived':False,'updatedAt':'DIAGNOSTIC_SECRET','project':{'id':s['project']['id']},'content':copy.deepcopy(s['issues'][0])})
        if s['mode']=='prepare-issues':s['issues'][0]['updatedAt']='DIAGNOSTIC_SECRET'
        if s['mode']=='prepare-fields':s['fields'].reverse()
        if s['mode'].startswith('prepare-history-'):s['statusUpdates'].append({'id':'DIAGNOSTIC_SECRET','body':'DIAGNOSTIC_SECRET '+s['mode'],'createdAt':'DIAGNOSTIC_SECRET','updatedAt':'DIAGNOSTIC_SECRET'})
        if s['mode']=='prepare-key-order':s['project']=dict(reversed(list(s['project'].items())))
        if s['mode']=='prepare-failure':save();fail()
    s['projection']=s['mode']=='delayed-project' and s.get('delay',0)>0
    if s['projection']:s['delay']-=1
    save();emit({'data':{'organization':{'projectV2':projected()}}})
for name in ['fields','items','views','workflows','repositories','statusUpdates']:
    if name+'(first:' in q:
        if name=='fields' and s['mode']=='prepare-incomplete' and s.get('starts',0)==2:
            project=projected();project[name]=page(s[name]);project[name]['pageInfo']['hasNextPage']=True
            emit({'data':{'node':project}})
        if name=='items' and s['mode'].startswith('snapshot') and s.get('snapshot',0)>0:
            project=projected()
            if v.get('after') is None:
                phantom={'id':'ITEM_phantom','type':'ISSUE','isArchived':False,'updatedAt':'old','project':{'id':s['project']['id']},'content':s['issues'][-1]}
                project[name]={'nodes':[phantom],'totalCount':2,'pageInfo':{'hasNextPage':True,'endCursor':'snapshot'}}
            else:
                if v['after']!='snapshot':fail()
                s['snapshot']-=1;s['conflicts']=s.get('conflicts',0)+1
                if s['mode']=='snapshot-context':s['statusUpdates'].append({'id':'U_foreign','body':'A human changes the planning boundary.','createdAt':'new','updatedAt':'new'})
                project[name]=page(s[name])
            save();emit({'data':{'node':project}})
        project=projected();project[name]=page(observed(name));emit({'data':{'node':project}})
if 'nodes(ids:' in q:emit({'data':{'nodes':[next((i for i in s['issues'] if i['id']==identity),None) for identity in v['ids']]}})
if v.get('id')=='IF_priority':emit({'data':{'node':{'id':'IF_priority','__typename':'IssueFieldSingleSelect'}}})
if v.get('id')=='R_repo':emit({'data':{'node':{'__typename':'Repository','id':'R_repo','nameWithOwner':'PerishLab/concord','owner':{'id':'O_lab'}}}})
fail()
"##;
