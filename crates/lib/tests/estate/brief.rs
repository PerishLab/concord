use concord_core::{
    Edit, Fact, Finish, Patch, Role, Seat, TASK_BRIEF_LIMIT, TASK_BRIEF_ROLE_BYTES,
};

#[tokio::test(flavor = "current_thread")]
async fn bounds() {
    let temp = tempfile::tempdir().expect("temporary Space");
    let estate = Seat::new(temp.path())
        .bootstrap()
        .await
        .expect("bootstrap estate");
    estate.manage("local").await.expect("manage Domain");
    let alpha = estate.start("local", "alpha").await.expect("start Task");
    let long = "界".repeat(TASK_BRIEF_ROLE_BYTES);
    let facts = [
        fact(Role::Goal, None, long.clone()),
        fact(Role::Constraint, Some(1), "excluded".to_string()),
        fact(Role::Focus, None, "current work".to_string()),
        fact(Role::Question, Some(1), "a".repeat(400)),
        fact(Role::Question, Some(2), "界".repeat(100)),
        fact(Role::Next, None, "next action".to_string()),
    ];
    estate
        .change(&Patch {
            version: 1,
            task: alpha.identity(),
            revision: 0,
            edits: facts
                .into_iter()
                .map(|fact| Edit::Create { fact })
                .collect(),
        })
        .await
        .expect("write current facts");

    let page = estate
        .task_brief("local", None)
        .await
        .expect("bounded brief");
    assert_eq!(page.schema, "concord.task-brief:v1");
    assert_eq!(page.returned, 1);
    let roles = &page.tasks[0].roles;
    assert_eq!(
        roles.keys().copied().collect::<Vec<_>>(),
        ["focus", "goal", "next", "question"]
    );
    let goal = &roles["goal"];
    assert_eq!(goal.bytes, long.len());
    assert!(goal.truncated);
    assert!(goal.facts[0].body.truncated);
    assert_eq!(goal.facts[0].body.text.len(), 510);
    let question = &roles["question"];
    assert_eq!(
        (question.total, question.returned, question.bytes),
        (2, 2, 700)
    );
    assert!(question.truncated);
    assert_eq!(question.facts[0].body.text.len(), 400);
    assert_eq!(question.facts[1].body.text.len(), 111);
    assert!(question.facts[1].body.truncated);
}

#[tokio::test(flavor = "current_thread")]
async fn pages() {
    let temp = tempfile::tempdir().expect("temporary Space");
    let estate = Seat::new(temp.path())
        .bootstrap()
        .await
        .expect("bootstrap estate");
    estate.manage("local").await.expect("manage Domain");
    let retired = estate
        .start("local", "retired")
        .await
        .expect("start retired Task");
    estate
        .finish(&Finish {
            task: retired.identity(),
            revision: 0,
            graph: 0,
            reason: "test fixture".to_string(),
        })
        .await
        .expect("retire Task");
    for index in 0..=TASK_BRIEF_LIMIT {
        estate
            .start("local", &format!("task-{index:03}"))
            .await
            .expect("start Task");
    }
    let first = estate.task_brief("local", None).await.expect("first page");
    assert_eq!(first.total, TASK_BRIEF_LIMIT + 1);
    assert_eq!(first.returned, TASK_BRIEF_LIMIT);
    assert_eq!(first.next.as_deref(), Some("task-063"));
    let second = estate
        .task_brief("local", first.next.as_deref())
        .await
        .expect("second page");
    assert_eq!(second.returned, 1);
    assert_eq!(second.tasks[0].task.name, "task-064");
    assert!(second.next.is_none());

    let error = estate
        .task_brief("local", Some("missing"))
        .await
        .expect_err("unknown cursor must refuse");
    assert_eq!(error.code(), "concord.task.brief_cursor");
}

fn fact(role: Role, rank: Option<i64>, body: String) -> Fact {
    Fact {
        key: None,
        role,
        rank,
        title: None,
        body,
        origin: None,
    }
}
