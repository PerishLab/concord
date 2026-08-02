use concord_core::{Memory, Root, Space};
use serde_json::Value;
use std::path::Path;
use std::process::{Command, Output};

#[test]
fn observation() {
    let fixture = tempfile::tempdir().expect("fixture");
    let root = Root::new(fixture.path()).expect("canonical root");
    let space = Space::new(root);
    space.domain_init("local", true).expect("initialize domain");
    space
        .task_start("local/observed", true)
        .expect("start task");
    let task = space.resolve("local/observed").expect("resolve task");
    Memory::new(&task)
        .init("# Observation\n")
        .expect("initialize memory");

    let report = fixture.path().join("atoms.jsonl");
    let trace = fixture.path().join("trace.json");

    let muted = run(
        fixture.path(),
        &["memory", "read", "local/observed"],
        &[
            ("CONCORD_LOCUS_ENABLED", "false"),
            (
                "CONCORD_LOCUS_REPORT_FILE",
                report.to_str().expect("report path"),
            ),
            (
                "CONCORD_LOCUS_TRACE_FILE",
                trace.to_str().expect("trace path"),
            ),
        ],
    );
    assert!(muted.status.success());
    assert!(!report.exists());

    let settings = [
        ("CONCORD_LOCUS_ENABLED", "true"),
        (
            "CONCORD_LOCUS_REPORT_FILE",
            report.to_str().expect("report path"),
        ),
        (
            "CONCORD_LOCUS_TRACE_FILE",
            trace.to_str().expect("trace path"),
        ),
    ];
    let plain = run(
        fixture.path(),
        &["domain", "list"],
        &[("CONCORD_LOCUS_ENABLED", "false")],
    );
    let outside = run(fixture.path(), &["domain", "list"], &settings);
    assert!(outside.status.success());
    assert_eq!(outside.status, plain.status);
    assert_eq!(outside.stdout, plain.stdout);
    assert_eq!(outside.stderr, plain.stderr);
    let atoms = read(&report);
    cycle(&atoms, "domain", 0);
    std::fs::remove_file(&report).expect("remove domain report");

    let observed = run(
        fixture.path(),
        &["memory", "read", "local/observed"],
        &settings,
    );
    assert!(
        observed.status.success(),
        "{}",
        String::from_utf8_lossy(&observed.stderr)
    );
    assert_eq!(observed.stdout, muted.stdout);
    assert_eq!(observed.stderr, muted.stderr);

    let trace: Value =
        serde_json::from_slice(&std::fs::read(&trace).expect("read trace")).expect("trace json");
    let trace = trace["key"].as_str().expect("trace key");
    let atoms = read(&report);
    cycle(&atoms, "memory", 0);
    let functions = atoms
        .iter()
        .filter(|atom| atom["source"]["function"].is_string())
        .collect::<Vec<_>>();
    assert!(functions.len() >= 4);
    let process = atoms
        .iter()
        .find(|atom| atom["payload"]["event"] == "cli.start")
        .expect("cli start")["context"]["locus.span"]
        .clone();
    assert!(
        functions
            .iter()
            .all(|atom| atom["context"]["locus.span"] != process)
    );
    assert!(
        functions
            .iter()
            .all(|atom| atom["context"]["locus.trace"] == trace)
    );

    for enter in functions
        .iter()
        .filter(|atom| atom["source"]["edge"] == "enter")
    {
        let returned = functions.iter().find(|candidate| {
            candidate["source"]["edge"] == "return"
                && candidate["source"]["module"] == enter["source"]["module"]
                && candidate["source"]["function"] == enter["source"]["function"]
        });
        let returned = returned.expect("normal return");
        assert_eq!(
            returned["context"]["locus.span"],
            enter["context"]["locus.span"]
        );
    }

    let report = fixture.path().join("collected.jsonl");
    let collected = run(
        fixture.path(),
        &["memory", "read", "local/observed"],
        &[
            ("CONCORD_LOCUS_ENABLED", "true"),
            (
                "CONCORD_LOCUS_REPORT_FILE",
                report.to_str().expect("collected path"),
            ),
            ("CODEX_THREAD_ID", "codex-thread"),
        ],
    );
    assert_eq!(collected.status, muted.status);
    assert_eq!(collected.stdout, muted.stdout);
    assert_eq!(collected.stderr, muted.stderr);

    let atoms = read(&report);
    assert!(
        atoms
            .iter()
            .all(|atom| atom["context"]["locus.trace"] == "codex-thread")
    );
    let collection = &atoms[0]["collections"][0];
    assert_eq!(collection["role"], "locus.trace");
    assert_eq!(collection["binding"], "codex.thread");
    assert_eq!(collection["collector"], "environment");
    assert_eq!(collection["selector"], "CODEX_THREAD_ID");

    let report = fixture.path().join("explicit.jsonl");
    let explicit = run(
        fixture.path(),
        &["memory", "read", "local/observed"],
        &[
            ("CONCORD_LOCUS_ENABLED", "true"),
            (
                "CONCORD_LOCUS_REPORT_FILE",
                report.to_str().expect("explicit path"),
            ),
            ("CONCORD_LOCUS_TRACE_ID", "explicit-trace"),
            ("CODEX_THREAD_ID", "ignored-thread"),
        ],
    );
    assert_eq!(explicit.status, muted.status);
    assert_eq!(explicit.stdout, muted.stdout);
    let atoms = read(&report);
    assert!(
        atoms
            .iter()
            .all(|atom| atom["context"]["locus.trace"] == "explicit-trace")
    );
    assert!(atoms[0]["collections"].is_null());

    let report = fixture.path().join("failed.jsonl");
    let baseline = run(
        fixture.path(),
        &["task", "show", "local/missing"],
        &[("CONCORD_LOCUS_ENABLED", "false")],
    );
    let failed = run(
        fixture.path(),
        &["task", "show", "local/missing"],
        &[
            ("CONCORD_LOCUS_ENABLED", "true"),
            (
                "CONCORD_LOCUS_REPORT_FILE",
                report.to_str().expect("failed path"),
            ),
            ("CODEX_THREAD_ID", "codex-thread"),
        ],
    );
    assert_eq!(failed.status, baseline.status);
    assert_eq!(failed.stdout, baseline.stdout);
    assert_eq!(failed.stderr, baseline.stderr);
    cycle(&read(&report), "task", 1);
}

#[test]
fn invalid() {
    let fixture = tempfile::tempdir().expect("fixture");
    let root = Root::new(fixture.path()).expect("canonical root");
    let space = Space::new(root);
    space.domain_init("local", true).expect("initialize domain");
    space.task_start("local/config", true).expect("start task");
    let task = space.resolve("local/config").expect("resolve task");
    Memory::new(&task)
        .init("# Config\n")
        .expect("initialize memory");

    let output = run(
        fixture.path(),
        &["memory", "read", "local/config"],
        &[("CONCORD_LOCUS_ENABLED", "not-a-boolean")],
    );
    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("concord locus config"));

    let output = run(
        fixture.path(),
        &["memory", "read", "local/config"],
        &[("CONCORD_LOCUS_ENABLED", "true")],
    );
    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("REPORT_FILE is required"));
}

fn run(root: &Path, arguments: &[&str], environment: &[(&str, &str)]) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_concord"));
    command
        .arg("--root")
        .arg(root)
        .args(arguments)
        .env_remove("CONCORD_LOCUS_ENABLED")
        .env_remove("CONCORD_LOCUS_REPORT_FILE")
        .env_remove("CONCORD_LOCUS_TRACE_FILE")
        .env_remove("CONCORD_LOCUS_TRACE_ID")
        .env_remove("CODEX_THREAD_ID");
    for (name, value) in environment {
        command.env(name, value);
    }
    command.output().expect("run concord")
}

fn read(path: &Path) -> Vec<Value> {
    std::fs::read_to_string(path)
        .expect("read atoms")
        .lines()
        .map(|line| serde_json::from_str(line).expect("atom json"))
        .collect()
}

fn cycle(atoms: &[Value], command: &str, code: i64) {
    let start = atoms
        .iter()
        .find(|atom| atom["payload"]["event"] == "cli.start")
        .expect("cli start");
    let finish = atoms
        .iter()
        .find(|atom| atom["payload"]["event"] == "cli.finish")
        .expect("cli finish");
    assert_eq!(start["payload"]["command"], command);
    assert_eq!(finish["payload"]["command"], command);
    assert_eq!(finish["payload"]["code"], code);
    assert_eq!(
        start["context"]["locus.trace"],
        finish["context"]["locus.trace"]
    );
    assert_eq!(
        start["context"]["locus.span"],
        finish["context"]["locus.span"]
    );
}
