use super::super::{fault, wire::Client};
use super::{build::Step, rest};
use concord_core::Result;
use serde_json::{Value, json};

pub(super) async fn perform(client: &Client<'_>, state: &Value, step: &Step) -> Result<Value> {
    if step.mutation == "rest:view" {
        let route = format!(
            "orgs/{}/projectsV2/{}/views",
            state["project"]["owner"]["login"]
                .as_str()
                .expect("qualified owner"),
            state["project"]["number"]
        );
        return rest::create(client, &route, &step.input).await;
    }
    let input = step.input.clone();
    let name = step.mutation;
    let kind = match name {
        "createProjectV2" => "CreateProjectV2Input",
        "updateProjectV2" => "UpdateProjectV2Input",
        "linkProjectV2ToRepository" => "LinkProjectV2ToRepositoryInput",
        "unlinkProjectV2FromRepository" => "UnlinkProjectV2FromRepositoryInput",
        "addProjectV2ItemById" => "AddProjectV2ItemByIdInput",
        "deleteProjectV2Item" => "DeleteProjectV2ItemInput",
        "createProjectV2Field" => "CreateProjectV2FieldInput",
        "updateProjectV2Field" => "UpdateProjectV2FieldInput",
        "deleteProjectV2Field" => "DeleteProjectV2FieldInput",
        "updateProjectV2View" => "UpdateProjectV2ViewInput",
        "deleteProjectV2View" => "DeleteProjectV2ViewInput",
        "deleteProjectV2Workflow" => "DeleteProjectV2WorkflowInput",
        "createProjectV2StatusUpdate" => "CreateProjectV2StatusUpdateInput",
        _ => unreachable!("compiled native operation"),
    };
    let query =
        format!("mutation($input:{kind}!){{change:{name}(input:$input){{clientMutationId}}}}");
    let data = client
        .read(&query, json!({"input":input}))
        .await
        .map_err(|error| {
            fault(
                "indeterminate",
                format!("native change may have completed; recover without replay: {error}"),
            )
        })?;
    if !data["change"].is_object() {
        return Err(fault(
            "indeterminate",
            "native change returned no receipt; recover without replay",
        ));
    }
    Ok(data["change"].clone())
}
