use concord_core::{Edit, Entry, Fact, Part, Patch, Role, Seat, Settle};

#[tokio::test(flavor = "current_thread")]
async fn changes() {
    let temp = tempfile::tempdir().expect("temporary Space");
    let estate = Seat::new(temp.path())
        .bootstrap()
        .await
        .expect("bootstrap estate");
    estate.manage("local").await.expect("manage Domain");
    estate.start("local", "typed").await.expect("start Task");
    let fact = |role, rank, body: &str| Fact {
        key: None,
        role,
        rank,
        title: None,
        body: body.to_string(),
        origin: None,
    };
    let first = Patch {
        version: 1,
        task: "local/typed".to_string(),
        revision: 0,
        edits: vec![
            Edit::Create {
                fact: fact(Role::Goal, None, "Expose the graph."),
            },
            Edit::Create {
                fact: fact(Role::Constraint, Some(1), "Keep exact authority."),
            },
            Edit::Create {
                fact: fact(Role::Constraint, Some(2), "Refuse stale writes."),
            },
            Edit::Create {
                fact: Fact {
                    key: None,
                    role: Role::Addition,
                    rank: Some(1),
                    title: Some("Legacy".to_string()),
                    body: "Opaque retained text.".to_string(),
                    origin: Some("authored".to_string()),
                },
            },
        ],
    };
    let current = estate.change(&first).await.expect("create facts");
    assert_eq!(current.task.revision, 1);
    assert_eq!(current.facts.len(), 4);
    let mut constraints = current
        .facts
        .iter()
        .filter(|fact| fact.role == Role::Constraint)
        .cloned()
        .collect::<Vec<_>>();
    let mut goal = current
        .facts
        .iter()
        .find(|fact| fact.role == Role::Goal)
        .expect("Goal")
        .clone();
    let addition = current
        .facts
        .iter()
        .find(|fact| fact.role == Role::Addition)
        .expect("Addition");
    constraints[0].rank = Some(2);
    constraints[1].rank = Some(1);
    goal.body = "Expose dependency problems.".to_string();
    let second = Patch {
        version: 1,
        task: "local/typed".to_string(),
        revision: 1,
        edits: vec![
            Edit::Set {
                fact: constraints[0].clone(),
            },
            Edit::Set {
                fact: constraints[1].clone(),
            },
            Edit::Set { fact: goal },
            Edit::End {
                role: Role::Addition,
                key: addition.key.expect("Addition key"),
            },
            Edit::Create {
                fact: fact(Role::Focus, None, "Ship typed transactions."),
            },
        ],
    };
    let current = estate.change(&second).await.expect("change facts");
    assert_eq!(current.task.revision, 2);
    assert_eq!(current.facts.len(), 4);
    assert_eq!(
        current
            .facts
            .iter()
            .filter(|fact| fact.role == Role::Constraint)
            .map(|fact| fact.rank.expect("rank"))
            .collect::<Vec<_>>(),
        vec![1, 2]
    );

    let error = match estate.change(&second).await {
        Ok(_) => panic!("stale change must refuse"),
        Err(error) => error,
    };
    assert_eq!(error.code(), "concord.task.stale");
    assert_eq!(
        estate
            .current("local/typed")
            .await
            .expect("current")
            .task
            .revision,
        2
    );

    let settle = Settle {
        version: 1,
        task: "local/typed".to_string(),
        revision: 2,
        phase: vec![
            Entry {
                key: None,
                part: Part::Outcome,
                rank: None,
                title: None,
                body: "Typed current state is atomic.".to_string(),
                origin: None,
            },
            Entry {
                key: None,
                part: Part::Decision,
                rank: Some(1),
                title: None,
                body: "Keep settle fully explicit.".to_string(),
                origin: None,
            },
            Entry {
                key: None,
                part: Part::Addition,
                rank: Some(1),
                title: Some("Context".to_string()),
                body: "Retain opaque phase text.".to_string(),
                origin: Some("authored".to_string()),
            },
        ],
        edits: vec![Edit::Create {
            fact: fact(Role::Next, None, "Build the migration."),
        }],
    };
    let settled = estate.settle(&settle).await.expect("settle Phase");
    assert_eq!(settled.current.task.revision, 3);
    assert_eq!(settled.phase.number, 1);
    assert_eq!(settled.phase.entries.len(), 3);
    assert_eq!(estate.phases("local/typed").await.expect("Phases").len(), 1);

    let invalid = Settle {
        version: 1,
        task: "local/typed".to_string(),
        revision: 3,
        phase: Vec::new(),
        edits: Vec::new(),
    };
    let error = match estate.settle(&invalid).await {
        Ok(_) => panic!("Outcome is required"),
        Err(error) => error,
    };
    assert_eq!(error.code(), "concord.phase.outcome");
    assert_eq!(
        estate
            .current("local/typed")
            .await
            .expect("current")
            .task
            .revision,
        3
    );
    assert_eq!(estate.phases("local/typed").await.expect("Phases").len(), 1);
}
