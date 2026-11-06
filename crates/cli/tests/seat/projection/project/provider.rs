use super::super::super::spawn;
use std::path::Path;
use std::process::Output;

pub(super) fn run(root: &Path, mode: &str, extra: &[&str]) -> Output {
    let provider = root.join(format!("provider-{mode}"));
    let source = format!(
        "#!/usr/bin/env python3\nMODE={}\nROOT={}\n{SCRIPT}",
        serde_json::to_string(mode).unwrap(),
        serde_json::to_string(root.to_str().unwrap()).unwrap()
    );
    plumb::testing::executable(&provider, source);
    spawn::concord(root)
        .args([
            "--space",
            "/missing-estate",
            "--json",
            "project",
            "survey",
            "PerishLab",
            "--project",
            "PerishLab/1",
            "--project",
            "PerishLab/2",
            "--github-command",
        ])
        .arg(provider)
        .args(extra)
        .output()
        .unwrap()
}

const SCRIPT: &str = r#"
import sys,json,pathlib,datetime
request=json.load(sys.stdin)
query=request['query']; variables=request['variables']
with open(pathlib.Path(ROOT)/'calls','a') as log:log.write(query+'\n')
assert not query.lstrip().startswith('mutation')
if MODE=='permission':sys.exit(1)
owner={'id':'O','login':'PerishLab','url':'https://github.com/PerishLab'}
def connection(nodes,more=False,total=None):
 return {'nodes':nodes,'totalCount':len(nodes) if total is None else total,'pageInfo':{'hasNextPage':more,'endCursor':'next' if more else None}}
def repository(number):
 return {'id':'R'+str(number),'nameWithOwner':'PerishLab/r'+str(number),'url':'https://github.com/PerishLab/r'+str(number),'owner':{'id':'O','login':'PerishLab'},'isArchived':False,'hasIssuesEnabled':True}
def issue(number,kind='Task'):
 return {'id':'I'+str(number),'number':number,'title':'Result '+str(number),'url':'https://github.com/PerishLab/r1/issues/'+str(number),'state':'OPEN','updatedAt':'same','issueType':{'id':'T','name':kind} if kind else None,'repository':{'id':'R1','nameWithOwner':'PerishLab/r1','owner':{'id':'O','login':'PerishLab'}}}
def project(number):
 return {'__typename':'ProjectV2','id':'P'+str(number),'number':number,'title':'Goal '+str(number),'url':'https://github.com/orgs/PerishLab/projects/'+str(number),'updatedAt':'same','closed':False,'public':False,'readme':'Goal, evidence and gap','shortDescription':'Goal','viewerCanUpdate':True,'owner':{'__typename':'Organization','id':'O','login':'PerishLab'}}
def fields(number):
 root={'id':'P'+str(number)}
 priority={'__typename':'ProjectV2SingleSelectField','id':'F_priority'+str(number),'name':'Priority','dataType':'SINGLE_SELECT','isIssueField':False,'project':root,'options':[{'id':name+str(number),'name':name} for name in ['Urgent','High','Medium','Low']]}
 periods=[]
 if MODE in ['ready','ambiguous-cycle']:
  date=datetime.datetime.now(datetime.timezone(datetime.timedelta(hours=8))).date().isoformat()
  periods=[{'id':'C','title':'Current','startDate':date,'duration':7}]
  if MODE=='ambiguous-cycle':periods.append(dict(periods[0],id='D'))
 cycle={'__typename':'ProjectV2IterationField','id':'F_cycle'+str(number),'name':'Cycle','dataType':'ITERATION','isIssueField':False,'project':root,'configuration':{'duration':7,'startDay':1,'iterations':periods,'completedIterations':[]}}
 text={'__typename':'ProjectV2Field','id':'F_contribution'+str(number),'name':'Contribution','dataType':'TEXT','isIssueField':False,'project':root}
 result=[priority,cycle,text]
 if MODE=='missing-fields':result=[text]
 if MODE=='ambiguous-fields':result.append(dict(cycle,id='another'))
 return result
def item(number):
 field={'id':'F_contribution'+str(number)}
 values=connection([{'__typename':'ProjectV2ItemFieldTextValue','field':field,'text':'Different benefit '+str(number)}])
 if MODE=='nested':values['pageInfo']={'hasNextPage':True,'endCursor':'next'}
 return {'id':'ITEM'+str(number),'type':'ISSUE','isArchived':False,'updatedAt':'same','project':{'id':'P'+str(number)},'content':None if MODE=='unknown-content' else dict(issue(1),__typename='Issue'),'fieldValues':values}
after=variables.get('after')
if 'repositories(first:' in query:
 nodes=[repository(1)]
 if MODE=='foreign':nodes[0]['owner']['id']='foreign'
 if MODE=='pages':nodes=[repository(2)] if after else nodes
 result=dict(owner,repositories=connection(nodes,MODE=='pages' and not after,2 if MODE=='pages' else None))
 print(json.dumps({'data':{'organization':result}}));sys.exit()
if 'issues(first:' in query:
 number=int(variables['id'][1:]);repo=repository(number)
 nodes=[issue(1),issue(2),issue(3,'Auto'),issue(4,None)] if number==1 else []
 more=False;total=None
 if MODE=='pages' and number==1:
  total=4;nodes=nodes[1:] if after else nodes[:1];more=not after
 if MODE=='repeat':nodes.append(nodes[0])
 if MODE=='issue-foreign':nodes[0]['repository']['id']='foreign'
 print(json.dumps({'data':{'node':dict(repo,issues=connection(nodes,more,total))}}));sys.exit()
if 'organization(login:' in query:
 number=variables['number'];result=project(number)
 if MODE=='project-foreign':result['owner']['id']='foreign'
 print(json.dumps({'data':{'organization':{'projectV2':result}}}));sys.exit()
number=int(variables['id'][1:]);result=project(number)
if 'fields(first:' in query:
 nodes=fields(number)
 if MODE=='field-page':result['fields']=connection(nodes[:1],True,3)
 else:result['fields']=connection(nodes)
elif 'items(first:' in query:
 assert 'ProjectV2ItemFieldTextValue{text field' in query
 rows=[item(number)]
 if MODE=='changed-issue':rows[0]['content']['updatedAt']='new'
 if MODE=='item-foreign':rows[0]['project']['id']='foreign'
 if MODE=='item-page':result['items']=connection(rows,True,2)
 else:result['items']=connection(rows)
elif MODE=='changed':result['updatedAt']='new'
print(json.dumps({'data':{'node':result}}))
"#;
