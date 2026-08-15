use concord_core::{Finish, Seat};
use serde_json::json;

#[tokio::test(flavor = "current_thread")]
async fn refusal() {
    let temp = tempfile::tempdir().expect("temporary Space");
    let estate = Seat::new(temp.path())
        .bootstrap()
        .await
        .expect("bootstrap estate");
    estate.manage("local").await.expect("manage local");
    estate.manage("other").await.expect("manage other");
    estate.start("local", "alpha").await.expect("start alpha");
    let gone = estate.start("local", "gone").await.expect("start gone");
    estate.start("other", "alpha").await.expect("start twin");

    let absent = estate.node("local/ghost").await.expect_err("absent Task");
    assert_eq!(absent.code(), "concord.task.absent");
    let details = absent.details().expect("absent details");
    assert_eq!(details["identity"], "local/ghost");
    assert_eq!(details["tasks"], json!(["local/alpha", "local/gone"]));
    assert_eq!(details["active"], 2);
    assert_eq!(details["retired"], 0);

    let foreign = estate
        .node("nosuch/ghost")
        .await
        .expect_err("absent Domain");
    assert_eq!(foreign.code(), "concord.task.absent");
    let details = foreign.details().expect("foreign details");
    assert_eq!(details["domains"], json!(["local", "other"]));
    assert!(details["tasks"].is_null());

    let twinned = estate.node("alpha").await.expect_err("ambiguous Task");
    assert_eq!(twinned.code(), "concord.task.ambiguous");
    let details = twinned.details().expect("ambiguous details");
    assert_eq!(details["identity"], "alpha");
    assert_eq!(details["tasks"], json!(["local/alpha", "other/alpha"]));

    let brief = estate
        .task_brief("nosuch", None)
        .await
        .expect_err("absent Domain brief");
    assert_eq!(brief.code(), "concord.domain.absent");
    let details = brief.details().expect("brief details");
    assert_eq!(details["domain"], "nosuch");
    assert_eq!(details["domains"], json!(["local", "other"]));

    estate
        .finish(&Finish {
            task: gone.identity(),
            reason: "folded into alpha".to_string(),
            revision: 0,
            graph: 0,
        })
        .await
        .expect("finish gone");
    let after = estate.node("local/ghost").await.expect_err("absent Task");
    let details = after.details().expect("absent details");
    assert_eq!(details["tasks"], json!(["local/alpha"]));
    assert_eq!(details["active"], 1);
    assert_eq!(details["retired"], 1);
}
