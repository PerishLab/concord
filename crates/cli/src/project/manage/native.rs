use super::super::{fault, wire::Client};
use super::{build::Step, model::Action};
use crate::runtime::github;
use concord_core::Result;
use runseal::github::{Alternative, Calendar, Outcome, Schema, Settings, Shape, Structure, view};
use serde_json::{Value, json};

pub(super) async fn perform(client: &Client, state: &Value, step: &Step) -> Result<Value> {
    let native = client.open()?;
    if let Some(change) = item(step)? {
        return encode(github::call("project.change", || native.change(&change)));
    }
    if let Some(schema) = schema(step)? {
        return encode(github::call("project.schema", || native.schema(&schema)));
    }
    if step.mutation == "rest:view" {
        return create(client, &native, state, step).await;
    }
    if let Some(view) = revision(step)? {
        return encode(github::call("project.view", || view.submit(&native)));
    }
    governance(&native, state, step)
}

fn item(step: &Step) -> Result<Option<runseal::github::Change>> {
    use runseal::github::{Action as Mutation, Change};
    let field = |name| -> Result<String> {
        step.input[name]
            .as_str()
            .map(str::to_owned)
            .ok_or_else(|| fault("schema", format!("native Project intent has no {name}")))
    };
    let action = match step.mutation {
        "addProjectV2ItemById" => Mutation::Add {
            content: field("contentId")?,
        },
        "deleteProjectV2Item" => Mutation::Remove {
            item: field("itemId")?,
        },
        "updateProjectV2ItemPosition" => Mutation::Move {
            item: field("itemId")?,
            after: step.input["afterId"].as_str().map(str::to_owned),
        },
        "updateProjectV2ItemFieldValue" => Mutation::Set {
            item: field("itemId")?,
            field: field("fieldId")?,
            value: serde_json::from_value(step.input["value"].clone())
                .map_err(|_| fault("schema", "unsupported native Project cell"))?,
        },
        "clearProjectV2ItemFieldValue" => Mutation::Clear {
            item: field("itemId")?,
            field: field("fieldId")?,
        },
        _ => return Ok(None),
    };
    Ok(Some(Change {
        project: field("projectId")?,
        action,
    }))
}

fn schema(step: &Step) -> Result<Option<Schema>> {
    let field = |name| -> Result<String> {
        step.input[name]
            .as_str()
            .map(str::to_owned)
            .ok_or_else(|| fault("schema", format!("native field intent has no {name}")))
    };
    let held = match step.mutation {
        "createProjectV2IssueField" => Schema::Attach {
            project: field("projectId")?,
            origin: field("issueFieldId")?,
        },
        "deleteProjectV2Field" => Schema::Remove {
            field: field("fieldId")?,
        },
        "createProjectV2Field" => Schema::Create {
            project: field("projectId")?,
            name: field("name")?,
            definition: shape(&step.input)?,
        },
        "updateProjectV2Field" => Schema::Update {
            field: field("fieldId")?,
            name: step.input["name"].as_str().map(str::to_owned),
            definition: update(&step.input)?,
        },
        _ => return Ok(None),
    };
    Ok(Some(held))
}

fn shape(input: &Value) -> Result<Shape> {
    match input["dataType"].as_str() {
        Some("TEXT") => Ok(Shape::Text),
        Some("NUMBER") => Ok(Shape::Number),
        Some("DATE") => Ok(Shape::Date),
        Some("SINGLE_SELECT") => Ok(Shape::Single(options(&input["singleSelectOptions"])?)),
        Some("MULTI_SELECT") => Ok(Shape::Multiple(options(&input["multiSelectOptions"])?)),
        Some("ITERATION") => Ok(Shape::Iteration(calendar(
            &input["iterationConfiguration"],
        )?)),
        _ => Err(fault("schema", "unsupported native Project field type")),
    }
}

fn update(input: &Value) -> Result<Option<Shape>> {
    if !input["singleSelectOptions"].is_null() {
        return Ok(Some(Shape::Single(options(&input["singleSelectOptions"])?)));
    }
    if !input["multiSelectOptions"].is_null() {
        return Ok(Some(Shape::Multiple(options(
            &input["multiSelectOptions"],
        )?)));
    }
    if !input["iterationConfiguration"].is_null() {
        return Ok(Some(Shape::Iteration(calendar(
            &input["iterationConfiguration"],
        )?)));
    }
    Ok(None)
}

fn options(value: &Value) -> Result<Vec<Alternative>> {
    serde_json::from_value(value.clone())
        .map_err(|_| fault("schema", "unsupported native select option definition"))
}

fn calendar(value: &Value) -> Result<Calendar> {
    serde_json::from_value(value.clone())
        .map_err(|_| fault("schema", "unsupported native iteration configuration"))
}

async fn create(
    client: &Client,
    native: &runseal::github::Client,
    state: &Value,
    step: &Step,
) -> Result<Value> {
    let owner = state["project"]["owner"]["login"]
        .as_str()
        .expect("qualified owner");
    let number = u32::try_from(state["project"]["number"].as_u64().unwrap_or_default())
        .map_err(|_| fault("identity", "invalid Project number"))?;
    let location = client.locator(owner, number).await?;
    let request: view::Create = serde_json::from_value(step.input.clone())
        .map_err(|_| fault("schema", "unsupported native view creation request"))?;
    let created = github::call("project.view.create", || request.submit(native, &location))
        .map_err(indeterminate)?
        .value;
    Ok(json!({"view": created.node}))
}

fn revision(step: &Step) -> Result<Option<view::Change>> {
    let held = match step.mutation {
        "deleteProjectV2View" => view::Change::Remove {
            view: step.input["viewId"]
                .as_str()
                .map(str::to_owned)
                .ok_or_else(|| fault("schema", "native view removal has no viewId"))?,
        },
        "updateProjectV2View" => {
            let layout = match step.input["layout"].as_str() {
                None => None,
                Some("TABLE_LAYOUT") => Some(view::Layout::Table),
                Some("BOARD_LAYOUT") => Some(view::Layout::Board),
                Some("ROADMAP_LAYOUT") => Some(view::Layout::Roadmap),
                Some(_) => return Err(fault("schema", "unsupported native view layout")),
            };
            let visible = match step.input.pointer("/configuration/visibleFieldIds") {
                None => None,
                Some(value) => Some(
                    serde_json::from_value(value.clone())
                        .map_err(|_| fault("schema", "unsupported native visible fields"))?,
                ),
            };
            view::Change::Update {
                view: step.input["viewId"]
                    .as_str()
                    .map(str::to_owned)
                    .ok_or_else(|| fault("schema", "native view update has no viewId"))?,
                name: step.input["name"].as_str().map(str::to_owned),
                layout,
                filter: step.input["filter"].as_str().map(str::to_owned),
                visible,
            }
        }
        _ => return Ok(None),
    };
    Ok(Some(held))
}

fn governance(native: &runseal::github::Client, state: &Value, step: &Step) -> Result<Value> {
    let project = || identifier(&state["project"]);
    let structure = match &step.check.action {
        Action::Create { title } => Structure::Create {
            owner: identifier(&state["owner"])?,
            title: title.clone(),
        },
        Action::Configure { changes } => Structure::Configure {
            project: project()?,
            settings: Settings {
                title: changes.title.clone(),
                description: changes.description.clone(),
                readme: changes.readme.clone(),
                public: changes.public,
                closed: None,
            },
        },
        Action::Link {
            repository,
            present,
        } => Structure::Relate {
            project: project()?,
            repository: repository.clone(),
            present: *present,
        },
        Action::Workflow { workflow } => {
            let id = project()?;
            return encode(github::call("project.retire", || {
                native.retire(&id, workflow)
            }));
        }
        Action::Record { body } => {
            let id = project()?;
            return encode(github::call("project.record", || native.record(&id, body)));
        }
        _ => unreachable!("compiled native operation"),
    };
    encode(github::call("project.structure", || {
        native.structure(&structure)
    }))
}

fn identifier(value: &Value) -> Result<String> {
    value["id"].as_str().map(str::to_owned).ok_or_else(|| {
        fault(
            "identity",
            "native Project intent has no stable object identity",
        )
    })
}

fn indeterminate(error: runseal::github::Error) -> concord_core::Error {
    fault(
        "indeterminate",
        format!(
            "view creation outcome is unknown; recover before another write: {}",
            github::describe(error)
        ),
    )
}

fn encode<T: serde::Serialize>(
    receipt: std::result::Result<runseal::github::Receipt<T>, runseal::github::Error>,
) -> Result<Value> {
    let receipt = receipt.map_err(|error| {
        let code = if error.outcome == Outcome::Unknown {
            "indeterminate"
        } else {
            "provider"
        };
        fault(code, github::describe(error))
    })?;
    serde_json::to_value(receipt.value).map_err(|_| {
        fault(
            "indeterminate",
            "native change receipt could not be encoded",
        )
    })
}
