use concord_core::activity::{Agent, Operator};
use concord_core::occupancy::{LEASE, Subject};
use concord_core::{Rename, Seat};
use fs2::FileExt;

#[tokio::test(flavor = "current_thread")]
async fn activity() {
    let temp = tempfile::tempdir().expect("temporary Space");
    let estate = Seat::new(temp.path())
        .bootstrap()
        .await
        .expect("bootstrap estate");
    estate.manage("local").await.expect("manage Domain");
    let task = estate.start("local", "alpha").await.expect("start Task");
    let codex = Operator {
        agent: Agent::Codex,
        session: "codex-one".to_string(),
    };
    let claude = Operator {
        agent: Agent::Claude,
        session: "claude-one".to_string(),
    };

    let first = estate
        .touch(&task, Some(&codex), "task.show")
        .expect("record first touch");
    assert!(first.recent.is_empty());
    let repeated = estate
        .touch(&task, Some(&codex), "phase.list")
        .expect("replace same session touch");
    assert!(repeated.recent.is_empty());
    let lock = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(
            temp.path()
                .join(format!(".concord/activity/{}.lock", task.key)),
        )
        .expect("activity lock");
    lock.lock_exclusive().expect("hold activity lock");
    let error = estate
        .touch(&task, Some(&codex), "task.show")
        .expect_err("busy activity must not wait");
    assert_eq!(error.code(), "concord.activity.busy");
    FileExt::unlock(&lock).expect("release activity lock");
    let concurrent = estate
        .touch(&task, Some(&claude), "member.status")
        .expect("record other session touch");
    assert_eq!(concurrent.recent.len(), 1);
    assert_eq!(concurrent.recent[0].agent, Some(Agent::Codex));
    assert_eq!(concurrent.recent[0].operation, "phase.list");
    assert_eq!(estate.node("local/alpha").await.expect("Task").revision, 0);

    let renamed = estate
        .rename(&Rename {
            task: task.identity(),
            name: "beta".to_string(),
            revision: 0,
        })
        .await
        .expect("rename Task");
    let after = estate
        .touch(&renamed, Some(&codex), "task.show")
        .expect("record after rename");
    assert_eq!(after.task, "local/beta");
    assert_eq!(estate.node("local/beta").await.expect("Task").revision, 1);
    let unknown = estate
        .touch(&renamed, None, "task.brief")
        .expect("record touch without session context");
    assert!(unknown.recent.is_empty());
    assert_eq!(unknown.current.agent, None);
    assert_eq!(unknown.current.session, None);
    assert_eq!(unknown.current.operation, "task.brief");

    let ledger = temp
        .path()
        .join(format!(".concord/activity/{}.json", task.key));
    let value: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&ledger).expect("activity ledger"))
            .expect("activity JSON");
    assert_eq!(value["version"], 2);
    let touches = value["touches"].as_array().expect("touches");
    assert_eq!(touches.len(), 3);
    let unknown = touches
        .iter()
        .find(|touch| touch["operation"] == "task.brief")
        .expect("touch without session context");
    assert!(unknown.get("agent").is_none());
    assert!(unknown.get("session").is_none());
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(ledger)
            .expect("activity metadata")
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(mode, 0o600);
    }
}

#[tokio::test(flavor = "current_thread")]
async fn occupancy() {
    let temp = tempfile::tempdir().expect("temporary Space");
    let estate = Seat::new(temp.path())
        .bootstrap()
        .await
        .expect("bootstrap estate");
    estate.manage("local").await.expect("manage Domain");
    estate.start("local", "alpha").await.expect("start Task");
    let codex = Operator {
        agent: Agent::Codex,
        session: "codex-one".to_string(),
    };
    let claude = Operator {
        agent: Agent::Claude,
        session: "claude-one".to_string(),
    };
    let grok = Operator {
        agent: Agent::Grok,
        session: "grok-one".to_string(),
    };
    let task = Subject::Task {
        task: "local/alpha".to_string(),
    };
    let member = Subject::Member {
        task: "local/alpha".to_string(),
        member: "repo".to_string(),
    };

    let first = estate
        .occupy(&codex, "task.change", std::slice::from_ref(&task))
        .expect("first holder");
    assert!(first.conflicts.is_empty());
    assert_eq!(first.lease, LEASE);
    let disjoint = estate
        .occupy(&claude, "member.prove", std::slice::from_ref(&member))
        .expect("disjoint holder");
    assert!(disjoint.conflicts.is_empty());
    let concurrent = estate
        .occupy(&claude, "task.rename", std::slice::from_ref(&task))
        .expect("intersecting holder");
    assert_eq!(concurrent.conflicts.len(), 1);
    assert_eq!(concurrent.conflicts[0].holder.session, "codex-one");
    assert_eq!(concurrent.conflicts[0].subjects, vec![task.clone()]);
    estate
        .occupy(&claude, "task.rehome", std::slice::from_ref(&task))
        .expect("renew same holder");

    let path = temp.path().join(".concord/occupancy/ledger.json");
    let mut ledger: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&path).expect("occupancy ledger"))
            .expect("occupancy JSON");
    assert_eq!(ledger["holders"].as_array().expect("holders").len(), 2);
    for holder in ledger["holders"].as_array_mut().expect("holders") {
        holder["heartbeat"] = 0.into();
    }
    std::fs::write(&path, serde_json::to_vec_pretty(&ledger).expect("ledger"))
        .expect("expire holders");
    let pruned = estate
        .occupy(&grok, "task.change", std::slice::from_ref(&task))
        .expect("prune expired holders");
    assert!(pruned.conflicts.is_empty());
    let ledger: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&path).expect("occupancy ledger"))
            .expect("occupancy JSON");
    assert_eq!(ledger["holders"].as_array().expect("holders").len(), 1);
    assert_eq!(estate.node("local/alpha").await.expect("Task").revision, 0);

    let lock = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(temp.path().join(".concord/occupancy/ledger.lock"))
        .expect("occupancy lock");
    lock.lock_exclusive().expect("hold occupancy lock");
    let error = estate
        .occupy(&codex, "task.change", std::slice::from_ref(&task))
        .expect_err("busy occupancy must not wait");
    assert_eq!(error.code(), "concord.occupancy.busy");
    FileExt::unlock(&lock).expect("release occupancy lock");
    std::fs::write(&path, b"not json").expect("corrupt occupancy");
    let error = estate
        .occupy(&codex, "task.change", &[task])
        .expect_err("corrupt occupancy is observable");
    assert_eq!(error.code(), "concord.occupancy.invalid");
}
