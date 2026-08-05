use concord_core::{Memory, MemoryBrief, Root, Space, TASK_BRIEF_LIMIT, TASK_BRIEF_SECTION_BYTES};

fn structured(focus: &str, next: &str) -> String {
    format!(
        "<!-- concord-memory:v1 -->\n# Brief\n\n## Goal\n\nShip.\n\n## Active constraints\n\nKeep the projection bounded.\n\n## Decisions in force\n\nReport facts only.\n\n## Current focus\n\n{focus}\n\n## Open questions\n\nNone.\n\n## Next step\n\n{next}\n"
    )
}

fn fixture() -> (tempfile::TempDir, Space) {
    let fixture = tempfile::tempdir().expect("fixture");
    let root = Root::new(fixture.path()).expect("canonical root");
    let space = Space::new(root);
    space.domain_init("local", true).expect("initialize domain");
    (fixture, space)
}

#[test]
fn brief_projects_registry_order_and_bounded_memory_evidence() {
    let (_fixture, space) = fixture();
    for name in ["alpha", "absent", "legacy", "broken"] {
        space
            .task_start(&format!("local/{name}"), true)
            .expect("start task");
    }
    let alpha = space.resolve("local/alpha").expect("alpha");
    let long = format!("{}中tail", "a".repeat(TASK_BRIEF_SECTION_BYTES - 1));
    Memory::new(&alpha)
        .init(&structured(&long, "Run the focused gate."))
        .expect("structured memory");
    let legacy = space.resolve("local/legacy").expect("legacy");
    Memory::new(&legacy)
        .init("# Legacy\n\nPreserve this body.\n")
        .expect("legacy memory");
    let broken = space.resolve("local/broken").expect("broken");
    std::fs::create_dir_all(broken.path().join(".task")).expect("broken memory root");
    std::fs::write(
        broken.path().join(".task/MAIN.md"),
        "<!-- concord-memory:v9 -->\n",
    )
    .expect("broken memory");

    let page = space.task_brief("local", None).expect("task brief");
    assert_eq!(page.schema, "concord.task-brief:v1");
    assert_eq!(page.total, 4);
    assert_eq!(page.returned, 4);
    assert_eq!(page.limits.tasks, TASK_BRIEF_LIMIT);
    assert_eq!(page.limits.section_bytes, TASK_BRIEF_SECTION_BYTES);
    assert_eq!(
        page.tasks
            .iter()
            .map(|entry| entry.task.name.as_str())
            .collect::<Vec<_>>(),
        ["alpha", "absent", "legacy", "broken"]
    );
    let MemoryBrief::Structured { sections, .. } = &page.tasks[0].memory else {
        panic!("alpha memory must be structured");
    };
    let focus = sections.get("focus").expect("focus preview");
    assert_eq!(focus.bytes, long.len());
    assert_eq!(focus.text.len(), TASK_BRIEF_SECTION_BYTES - 1);
    assert!(focus.truncated);
    assert_eq!(
        sections.get("next").expect("next preview").text,
        "Run the focused gate."
    );
    assert!(matches!(page.tasks[1].memory, MemoryBrief::Absent));
    assert!(matches!(page.tasks[2].memory, MemoryBrief::Legacy { .. }));
    assert!(matches!(
        page.tasks[3].memory,
        MemoryBrief::Unavailable { .. }
    ));
}

#[test]
fn brief_pages_at_a_fixed_limit_and_refuses_unknown_cursors() {
    let (_fixture, space) = fixture();
    for index in 0..=TASK_BRIEF_LIMIT {
        space
            .task_start(&format!("local/t{index:03}"), true)
            .expect("start task");
    }

    let first = space.task_brief("local", None).expect("first page");
    assert_eq!(first.total, TASK_BRIEF_LIMIT + 1);
    assert_eq!(first.returned, TASK_BRIEF_LIMIT);
    assert_eq!(first.next.as_deref(), Some("t063"));
    let second = space
        .task_brief("local", first.next.as_deref())
        .expect("second page");
    assert_eq!(second.returned, 1);
    assert_eq!(second.tasks[0].task.name, "t064");
    assert!(second.next.is_none());

    let error = space
        .task_brief("local", Some("missing"))
        .expect_err("unknown cursor must fail");
    assert_eq!(error.code(), "task.brief_cursor");
}
