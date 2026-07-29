use concord_core::{Root, Space, Status};
use tempfile::TempDir;

struct Fixture {
    _temp: TempDir,
    space: Space,
}

impl Fixture {
    fn new() -> Self {
        let temp = tempfile::tempdir().expect("temporary domain space");
        let root = Root::new(temp.path()).expect("canonical root");
        let space = Space::new(root);
        space.domain_init("local", true).expect("initialize domain");
        Self { space, _temp: temp }
    }

    fn task(&self, name: &str) {
        self.space
            .task_start(&format!("local/{name}"), true)
            .expect("start task");
    }
}

#[test]
fn one_sweep_reports_one_host_memory_sample() {
    let fixture = Fixture::new();
    for name in ["first", "second", "third"] {
        fixture.task(name);
    }

    let audit = fixture.space.audit().expect("audit space");
    assert_eq!(audit.resources.len(), 3);
    let held = audit
        .resources
        .iter()
        .map(|task| serde_json::to_string(&task.host_memory).expect("encode observation"))
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(
        held.len(),
        1,
        "every task in one sweep carries the same host reading: {held:?}"
    );
}

#[test]
fn a_task_that_holds_nothing_stays_ok_whatever_the_host_reports() {
    let fixture = Fixture::new();
    fixture.task("empty");

    let audit = fixture.space.audit().expect("audit space");
    let task = audit.resources.first().expect("one task observed");
    assert_eq!(task.task.status, Status::Ok);
    assert!(task.members.is_empty());
    assert!(task.resources.is_empty());
    assert_eq!(
        task.status,
        Status::Ok,
        "status follows the task's own footprints, not the machine it runs on"
    );
}
