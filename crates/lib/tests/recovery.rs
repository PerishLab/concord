use concord_core::{Memory, Root, Space};
use tempfile::TempDir;

struct Fixture {
    _temp: TempDir,
    space: Space,
}

impl Fixture {
    fn new() -> Self {
        let temp = tempfile::tempdir().expect("temporary domain space");
        let root = Root::new(temp.path()).expect("canonical root");
        Self {
            space: Space::new(root),
            _temp: temp,
        }
    }

    fn task(&self, name: &str) {
        self.space
            .domain_init("local", true)
            .expect("initialize domain");
        self.space
            .task_start(&format!("local/{name}"), true)
            .expect("start task");
    }

    #[cfg(unix)]
    fn seat(&self, name: &str) -> std::path::PathBuf {
        self.task(name);
        let task = self
            .space
            .resolve(&format!("local/{name}"))
            .expect("resolve task");
        let memory = Memory::new(&task);
        memory
            .init("# Current objective\n")
            .expect("initialize memory");
        memory.allocate("evidence").expect("allocate resource seat")
    }
}

#[test]
#[cfg(unix)]
fn an_out_of_band_symlink_does_not_strand_its_resource_seat() {
    let fixture = Fixture::new();
    let seat = fixture.seat("recovery");
    let task = fixture
        .space
        .resolve("local/recovery")
        .expect("resolve task");

    let payload = seat.join("payload");
    std::fs::create_dir(&payload).expect("create payload directory");
    std::fs::write(payload.join("real.txt"), "evidence\n").expect("write payload file");
    std::os::unix::fs::symlink(payload.join("real.txt"), payload.join("link.txt"))
        .expect("plant an out-of-band symbolic link");

    let audit = task.audit().expect("audit task");
    let link = audit
        .faults
        .iter()
        .find(|fault| fault.kind == "resource")
        .expect("audit reports the out-of-band symbolic link");
    assert!(
        !link.gates(),
        "a link under .task/ is hygiene, not one of the four agreed surfaces"
    );
    assert!(audit.agrees(), "the four surfaces still agree");
    assert!(!audit.ok(), "audit still reports the finding");

    let planned = fixture
        .space
        .normalize("local/recovery", false)
        .expect("plan normalize");
    assert!(
        planned.actions.iter().any(|action| action.verb == "skip"),
        "the plan the operator reads names the link before anything runs"
    );
    fixture
        .space
        .normalize("local/recovery", true)
        .expect("normalize completes across the link");

    fixture
        .space
        .resource_remove("local/recovery", "evidence", true)
        .expect("the seat holding the link is removable");
    assert!(!seat.exists(), "the offending seat is gone");
    assert!(
        task.audit().expect("re-audit").ok(),
        "removing the tree clears the finding it carried"
    );
}

#[test]
fn narrowing_the_gate_does_not_open_it_for_a_missing_task_root() {
    let fixture = Fixture::new();
    fixture.task("structural");
    let task = fixture
        .space
        .resolve("local/structural")
        .expect("resolve task");
    std::fs::remove_dir(task.path()).expect("remove the declared task root");

    let audit = task.audit().expect("audit task");
    assert!(
        audit.faults.iter().any(|fault| fault.gates()),
        "a missing task root gates mutation"
    );
    assert!(!audit.agrees());

    let refused = Memory::new(&task)
        .init("# Current objective\n")
        .expect_err("mutation is refused while a surface disagrees");
    assert!(
        refused.to_string().contains("protocol mismatch"),
        "refusal names the mismatch: {refused}"
    );
}
