use concord_core::{Registry, Root, Space};

struct Fixture {
    _temp: tempfile::TempDir,
    space: Space,
}

impl Fixture {
    fn new() -> Self {
        let temp = tempfile::tempdir().expect("temporary domain space");
        let space = Space::new(Root::new(temp.path()).expect("canonical root"));
        space.domain_init("local", true).expect("initialize domain");
        Self { _temp: temp, space }
    }

    fn task(&self, name: &str) {
        self.space
            .task_start(&format!("local/{name}"), true)
            .expect("start task");
    }
}

#[test]
fn todo_create_link_is_idempotent_and_source_finish_hands_off() {
    let fixture = Fixture::new();
    fixture.task("source");

    let planned = fixture
        .space
        .task_todo_add("local/source", "future", false)
        .expect("plan todo");
    assert_eq!(
        planned
            .actions
            .iter()
            .map(|action| action.verb.as_str())
            .collect::<Vec<_>>(),
        ["create", "link"]
    );
    fixture
        .space
        .task_todo_add("local/source", "future", true)
        .expect("create and link todo");
    let source = fixture.space.resolve("local/source").expect("source");
    assert_eq!(source.task().todo, ["future"]);
    assert!(fixture.space.resolve("local/future").is_ok());

    let registry = source.domain().registry_path();
    let before = std::fs::read(&registry).expect("registry before idempotent add");
    let repeated = fixture
        .space
        .task_todo_add("local/source", "future", true)
        .expect("repeat todo");
    assert_eq!(repeated.actions[0].verb, "keep");
    assert_eq!(
        std::fs::read(&registry).expect("registry after idempotent add"),
        before
    );

    let finish = fixture
        .space
        .task_finish("local/source", false)
        .expect("plan source finish");
    assert_eq!(finish.actions[0].verb, "handoff");
    fixture
        .space
        .task_finish("local/source", true)
        .expect("finish source");
    assert!(fixture.space.resolve("local/source").is_err());
    assert!(fixture.space.resolve("local/future").is_ok());
}

#[test]
fn incoming_todo_blocks_target_finish_until_explicit_unlink() {
    let fixture = Fixture::new();
    fixture.task("source");
    fixture.task("target");
    fixture
        .space
        .task_todo_add("local/source", "target", true)
        .expect("link existing task");

    let error = fixture
        .space
        .task_finish("local/target", false)
        .expect_err("incoming todo must block finish");
    assert!(error.to_string().contains("source"));

    fixture
        .space
        .task_todo_remove("local/source", "target", true)
        .expect("unlink target");
    fixture
        .space
        .task_finish("local/target", true)
        .expect("finish unlinked target");
}

#[test]
fn todo_refuses_self_cross_domain_and_linked_rehome() {
    let fixture = Fixture::new();
    fixture.task("source");
    fixture
        .space
        .domain_init("other", true)
        .expect("initialize other domain");
    fixture
        .space
        .task_start("other/future", true)
        .expect("start cross-domain target");

    assert!(
        fixture
            .space
            .task_todo_add("local/source", "source", false)
            .is_err()
    );
    assert!(
        fixture
            .space
            .task_todo_add("local/source", "other/future", false)
            .is_err()
    );

    fixture
        .space
        .task_todo_add("local/source", "future", true)
        .expect("add local future");
    assert!(
        fixture
            .space
            .task_rehome("local/source", "other", false)
            .is_err()
    );
    assert!(
        fixture
            .space
            .task_rehome("local/future", "other", false)
            .is_err()
    );
}

#[test]
fn rename_rewrites_incoming_todos() {
    let fixture = Fixture::new();
    fixture.task("source");
    fixture.task("before");
    fixture
        .space
        .task_todo_add("local/source", "before", true)
        .expect("link target");
    fixture
        .space
        .task_rename("local/before", "after", true)
        .expect("rename target");

    let source = fixture.space.resolve("local/source").expect("source");
    assert_eq!(source.task().todo, ["after"]);
    assert!(fixture.space.resolve("local/before").is_err());
    assert!(fixture.space.resolve("local/after").is_ok());
}

#[test]
fn registry_refuses_noncanonical_or_dangling_todos() {
    for todo in ["[\"missing\"]", "[\"source\"]", "[\"target\", \"target\"]"] {
        let text = format!(
            "version = 3\n\n[[task]]\nname = \"source\"\ntodo = {todo}\n\n[[task]]\nname = \"target\"\n"
        );
        let registry: Registry = toml::from_str(&text).expect("parse registry");
        assert!(registry.validate().is_err(), "todo {todo} must refuse");
    }

    let text = "version = 2\n\n[[task]]\nname = \"source\"\ntodo = [\"target\"]\n\n[[task]]\nname = \"target\"\n";
    let registry: Registry = toml::from_str(text).expect("parse version 2 registry");
    assert!(registry.validate().is_err());
}

#[test]
fn failed_todo_add_rolls_back_the_created_target_root() {
    let fixture = Fixture::new();
    fixture.task("source");
    let domain = fixture.space.domain("local").expect("domain");
    let collision = domain
        .tasks_path()
        .join(format!(".tasks.toml.concord-{}", std::process::id()));
    std::fs::create_dir(&collision).expect("block atomic registry temporary");

    let error = fixture
        .space
        .task_todo_add("local/source", "future", true)
        .expect_err("registry write must fail");
    assert!(!error.to_string().is_empty());
    assert!(!domain.tasks_path().join("future").exists());
    let source = fixture.space.resolve("local/source").expect("source");
    assert!(source.task().todo.is_empty());
}
