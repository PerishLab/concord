use concord_core::{Add, MigrationClaim, Root, Space};
use std::path::Path;
use std::process::Command;

#[test]
fn version_one_migration_requires_and_normalizes_every_active_member_claim() {
    let temp = tempfile::tempdir().expect("temporary domain space");
    let space = Space::new(Root::new(temp.path()).expect("canonical root"));
    space.domain_init("local", true).expect("initialize domain");
    for task in ["first", "second"] {
        space
            .task_start(&format!("local/{task}"), true)
            .expect("start task");
    }
    let source = temp.path().join("local/repo");
    std::fs::create_dir_all(&source).expect("create source");
    git(&source, &["init", "-b", "main"]);
    git(&source, &["config", "user.name", "Concord Test"]);
    git(
        &source,
        &["config", "user.email", "concord@example.invalid"],
    );
    std::fs::write(source.join("README.md"), "fixture\n").expect("write source");
    git(&source, &["add", "README.md"]);
    git(&source, &["commit", "-m", "init"]);
    for (task, write) in [("first", "crates/first"), ("second", "crates/second")] {
        space
            .member_add(
                Add {
                    task: &format!("local/{task}"),
                    name: "repo",
                    source: &source,
                    branch: None,
                    orphan: false,
                    write: &[write.to_string()],
                },
                true,
            )
            .expect("add member");
    }

    let domain = space.domain("local").expect("resolve domain");
    let path = domain.registry_path();
    let mut registry: toml::Value =
        toml::from_str(&std::fs::read_to_string(&path).expect("read registry"))
            .expect("parse registry");
    registry["version"] = toml::Value::Integer(1);
    for task in registry["task"].as_array_mut().expect("tasks") {
        for member in task["repo"].as_array_mut().expect("members") {
            member.as_table_mut().expect("member table").remove("write");
        }
    }
    std::fs::write(&path, toml::to_string_pretty(&registry).expect("encode v1"))
        .expect("write v1 registry");

    let first = MigrationClaim {
        task: "first".into(),
        member: "repo".into(),
        write: "crates/first".into(),
    };
    let error = space
        .domain_migrate("local", std::slice::from_ref(&first), false)
        .expect_err("missing member claim must fail");
    assert!(error.to_string().contains("second/repo"));

    let second = MigrationClaim {
        task: "second".into(),
        member: "repo".into(),
        write: "crates/second/deep".into(),
    };
    space
        .domain_migrate("local", &[first, second], true)
        .expect("migrate registry");
    let registry = domain.registry().expect("read migrated registry");
    assert_eq!(registry.version, 3);
    assert_eq!(registry.task[0].repo[0].write, ["crates/first"]);
    assert_eq!(registry.task[1].repo[0].write, ["crates/second/deep"]);
}

#[test]
fn version_two_migration_is_claim_free_and_old_version_range_refuses_three() {
    let temp = tempfile::tempdir().expect("temporary domain space");
    let space = Space::new(Root::new(temp.path()).expect("canonical root"));
    space.domain_init("local", true).expect("initialize domain");
    space
        .task_start("local/source", true)
        .expect("start source");
    let domain = space.domain("local").expect("resolve domain");
    let path = domain.registry_path();
    let mut registry: toml::Value =
        toml::from_str(&std::fs::read_to_string(&path).expect("read registry"))
            .expect("parse registry");
    registry["version"] = toml::Value::Integer(2);
    std::fs::write(&path, toml::to_string_pretty(&registry).expect("encode v2"))
        .expect("write v2 registry");

    let claim = MigrationClaim {
        task: "source".into(),
        member: "repo".into(),
        write: ".".into(),
    };
    let error = space
        .domain_migrate("local", &[claim], false)
        .expect_err("version 2 migration must reject claims");
    assert!(error.to_string().contains("does not accept member claims"));

    space
        .domain_migrate("local", &[], true)
        .expect("migrate version 2 registry");
    let raw = std::fs::read_to_string(&path).expect("read migrated registry");
    let migrated = domain.registry().expect("parse version 3 registry");
    assert_eq!(migrated.version, 3);
    assert!(
        !legacy(&raw),
        "the v0.8.0 registry version range must reject version 3"
    );
}

fn legacy(text: &str) -> bool {
    let value: toml::Value = toml::from_str(text).expect("parse legacy probe");
    value
        .get("version")
        .and_then(toml::Value::as_integer)
        .is_some_and(|version| matches!(version, 1 | 2))
}

fn git(root: &Path, args: &[&str]) {
    let status = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .status()
        .expect("run git");
    assert!(status.success(), "git {} failed", args.join(" "));
}
