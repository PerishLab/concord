use concord_core::{MAX_MAIN_BYTES, Memory, Root, Space};

fn main(constraints: &str) -> String {
    format!(
        "<!-- concord-memory:v1 -->\n# Brief\n\n## Goal\n\nShip.\n\n## Active constraints\n\n{constraints}\n\n## Decisions in force\n\nUse CAS.\n\n## Current focus\n\nParser.\n\n## Open questions\n\nNone.\n\n## Next step\n\nTest.\n"
    )
}

fn memory() -> (tempfile::TempDir, Space) {
    let fixture = tempfile::tempdir().expect("fixture");
    let root = Root::new(fixture.path()).expect("canonical root");
    let space = Space::new(root);
    space.domain_init("local", true).expect("initialize domain");
    space.task_start("local/format", true).expect("start task");
    (fixture, space)
}

#[test]
fn commonmark_containers_keep_nested_h2_text_inside_section_bodies() {
    let (_fixture, space) = memory();
    let task = space.resolve("local/format").expect("resolve task");
    let memory = Memory::new(&task);
    let source =
        main("> ## Quoted\n\n```markdown\n## Fenced\n```\n\n- item\n\n  ## Listed\n\n### Nested");
    memory.init(&source).expect("initialize structured memory");
    let projection = memory
        .read_sections(&["constraints".to_string()])
        .expect("project constraints");
    assert!(projection.content.contains("> ## Quoted"));
    assert!(projection.content.contains("## Fenced"));
    assert!(projection.content.contains("## Listed"));
}

#[test]
fn structured_text_and_closed_h2_schema_quick_fail_before_init() {
    let (_fixture, space) = memory();
    let task = space.resolve("local/format").expect("resolve task");
    let memory = Memory::new(&task);
    let unknown = main("Keep bytes.").replace("## Next step", "## Later");
    assert_eq!(
        memory.init(&unknown).expect_err("unknown H2").code(),
        "memory.schema"
    );
    let reordered = main("Keep bytes.")
        .replace("## Goal", "## Temporary")
        .replace("## Active constraints", "## Goal")
        .replace("## Temporary", "## Active constraints");
    assert_eq!(
        memory.init(&reordered).expect_err("reordered H2").code(),
        "memory.schema"
    );
    assert_eq!(
        memory
            .init(&main("Keep bytes.").replace('\n', "\r\n"))
            .expect_err("CRLF")
            .code(),
        "memory.text_newline"
    );
    assert_eq!(
        memory
            .init(&format!("\u{feff}{}", main("Keep bytes.")))
            .expect_err("BOM")
            .code(),
        "memory.text_bom"
    );
    assert_eq!(
        memory
            .init(main("Keep bytes.").trim_end())
            .expect_err("missing final LF")
            .code(),
        "memory.text_final_newline"
    );
    assert_eq!(
        memory
            .init(&format!("# Legacy\n{}", "x".repeat(MAX_MAIN_BYTES)))
            .expect_err("byte limit")
            .code(),
        "memory.limit_bytes"
    );
    assert_eq!(
        memory
            .init(&format!("# Legacy\n{}", "x\n".repeat(400)))
            .expect_err("line limit")
            .code(),
        "memory.limit_lines"
    );
    assert!(
        !memory.root().exists(),
        "refused init leaves no memory root"
    );
}

#[test]
fn raw_read_ceiling_quick_fails_without_hiding_audit_evidence() {
    let (_fixture, space) = memory();
    let task = space.resolve("local/format").expect("resolve task");
    let memory = Memory::new(&task);
    memory.init("# Legacy\n").expect("initialize memory");
    std::fs::write(memory.main(), vec![b'x'; 4 * 1024 * 1024 + 1]).expect("oversize memory");
    private_file(&memory.main());
    assert_eq!(
        memory.read().expect_err("raw read ceiling").code(),
        "memory.read_limit"
    );
    let audit = task.audit().expect("streaming audit");
    assert!(
        audit
            .faults
            .iter()
            .any(|fault| fault.kind == "memory" && fault.message.contains("maximum"))
    );
    assert!(audit.agrees(), "memory hygiene does not break agreement");
}

#[cfg(unix)]
fn private_file(path: &std::path::Path) {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600)).expect("private file");
}

#[cfg(not(unix))]
fn private_file(_: &std::path::Path) {}
