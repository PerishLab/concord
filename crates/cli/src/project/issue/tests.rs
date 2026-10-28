use super::{fixture::World, observe};
use serde_json::{Value, json};

#[tokio::test]
async fn parallel() {
    let world = World::new("parallel");
    let (result, collections) = world.run(9, 20).await;
    result.unwrap();
    let state = world.state();
    assert_eq!(state["peak"], 4);
    assert_eq!(state["active"], 0);
    assert_eq!(
        &state["confirmed"].as_array().unwrap()[..4],
        &[json!(4), json!(3), json!(2), json!(1)]
    );
    for (index, item) in collections["items"]["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .enumerate()
    {
        let field = &item["content"]["issueFields"]["nodes"][0];
        assert_eq!(field["value"], index + 1);
        assert_eq!(field["project_fields"], json!(["F_attached"]));
    }
    let events = state["events"].as_array().unwrap();
    let fifth = events
        .iter()
        .position(|event| *event == json!(["start", 5]))
        .unwrap();
    for number in 1..=4 {
        assert!(events[..fifth].contains(&json!(["confirmed", number])));
    }
}

#[tokio::test]
async fn drain() {
    let world = World::new("refused");
    let (result, _) = world.run(8, 20).await;
    assert_eq!(result.unwrap_err().code(), "concord.project.provider");
    let state = world.state();
    assert_eq!(state["active"], 0);
    let mut confirmed = state["confirmed"].as_array().unwrap().clone();
    confirmed.sort_by_key(|number| number.as_u64());
    assert_eq!(confirmed, vec![json!(2), json!(3), json!(4)]);
    assert!(
        !state["events"]
            .as_array()
            .unwrap()
            .contains(&json!(["start", 5]))
    );
}

#[tokio::test]
async fn pages() {
    let world = World::new("pages");
    let (result, collections) = world.run(1, 20).await;
    result.unwrap();
    let fields = &collections["items"]["nodes"][0]["content"]["issueFields"];
    assert_eq!(fields["complete"], true);
    assert_eq!(fields["pages"].as_array().unwrap().len(), 2);
    assert_eq!(fields["nodes"][0]["project_fields"], json!([]));
    assert_eq!(fields["nodes"][41]["project_fields"], json!(["F_attached"]));
    assert_eq!(
        world.state()["events"],
        json!([["start", 1], ["page", 1], ["page", 1], ["confirmed", 1]])
    );
    let (result, collections) = World::new("pages").run(1, 1).await;
    result.unwrap();
    assert_eq!(
        collections["items"]["nodes"][0]["content"]["issueFields"]["complete"],
        false
    );
    let (result, collections) = World::new("empty").run(1, 20).await;
    result.unwrap();
    assert_eq!(
        collections["items"]["nodes"][0]["content"]["issueFields"]["nodes"],
        json!([])
    );
}

#[tokio::test]
async fn refuse() {
    for (mode, code) in [
        ("partial", "schema"),
        ("repeat", "changed"),
        ("type", "schema"),
        ("node", "changed"),
        ("repository", "changed"),
        ("stamp", "changed"),
        ("graphql", "provider"),
        ("timeout", "provider"),
    ] {
        let world = World::new(mode);
        let (result, _) = world.run(4, 20).await;
        assert_eq!(
            result.unwrap_err().code(),
            format!("concord.project.{code}"),
            "{mode}"
        );
        #[cfg(target_os = "linux")]
        if mode == "timeout" {
            world.quiet().await;
        }
    }
}

#[tokio::test]
async fn absent() {
    let world = World::new("refused");
    let issue = json!({"content":{"__typename":"Issue"}});
    let mut collections = json!({"fields":{"nodes":[]},"items":{"nodes":[issue]}});
    observe(&world.client(), &mut collections, 20)
        .await
        .unwrap();
    assert_eq!(
        collections["items"]["nodes"][0]["content"]["issueFields"],
        Value::Null
    );
    collections["fields"]["nodes"] = json!([{"authority":"issue"}]);
    collections["items"]["nodes"][0]["content"] = Value::Null;
    observe(&world.client(), &mut collections, 20)
        .await
        .unwrap();
}
