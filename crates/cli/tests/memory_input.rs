use concord_core::{Memory, Root, Space};
use serde_json::Value;
use std::path::Path;
use std::process::{Command, Output};

fn run(root: &Path, arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_concord"))
        .arg("--root")
        .arg(root)
        .args(arguments)
        .output()
        .expect("run concord")
}

#[test]
fn file_inputs_are_consumed_by_default_and_keep_file_is_explicit() {
    let fixture = tempfile::tempdir().expect("fixture");
    let root = Root::new(fixture.path()).expect("canonical root");
    let space = Space::new(root);
    space.domain_init("local", true).expect("initialize domain");
    space.task_start("local/input", true).expect("start task");
    let task = space.resolve("local/input").expect("resolve task");
    let memory = Memory::new(&task);
    memory.init("# Initial\n").expect("initialize memory");

    let first = memory.read().expect("first revision");
    let input = fixture.path().join("NEXT.md");
    std::fs::write(&input, "# Next\n").expect("write input");
    let output = run(
        fixture.path(),
        &[
            "--json",
            "memory",
            "write",
            "local/input",
            "--expect",
            &first.revision,
            "--file",
            input.to_str().expect("input path"),
        ],
    );
    assert!(output.status.success());
    assert!(!input.exists(), "successful input is consumed");

    let current = memory.read().expect("current revision");
    std::fs::write(&input, &current.content).expect("write noop input");
    let output = run(
        fixture.path(),
        &[
            "--json",
            "memory",
            "write",
            "local/input",
            "--expect",
            &current.revision,
            "--file",
            input.to_str().expect("input path"),
        ],
    );
    let output: Value = serde_json::from_slice(&output.stdout).expect("noop JSON");
    assert_eq!(output["result"]["changed"], false);
    assert_eq!(output["result"]["revision"], current.revision);
    assert!(!input.exists(), "noop still consumes its successful input");

    std::fs::write(&input, "# Kept\n").expect("write kept input");
    let output = run(
        fixture.path(),
        &[
            "memory",
            "write",
            "local/input",
            "--expect",
            &current.revision,
            "--file",
            input.to_str().expect("input path"),
            "--keep-file",
        ],
    );
    assert!(output.status.success());
    assert!(input.is_file(), "--keep-file retains the source");
}

#[test]
fn memory_init_accepts_explicit_stdin() {
    use std::io::Write;
    use std::process::Stdio;

    let fixture = tempfile::tempdir().expect("fixture");
    let root = Root::new(fixture.path()).expect("canonical root");
    let space = Space::new(root);
    space.domain_init("local", true).expect("initialize domain");
    space.task_start("local/stdin", true).expect("start task");
    let mut child = Command::new(env!("CARGO_BIN_EXE_concord"))
        .arg("--root")
        .arg(fixture.path())
        .args(["memory", "init", "local/stdin", "--file", "-"])
        .stdin(Stdio::piped())
        .spawn()
        .expect("run init");
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(b"# From stdin\n")
        .expect("write stdin");
    assert!(child.wait().expect("wait").success());
    let task = space.resolve("local/stdin").expect("resolve task");
    assert_eq!(
        Memory::new(&task).read().expect("read memory").content,
        "# From stdin\n"
    );
}

#[test]
fn rejected_file_inputs_are_retained_and_managed_memory_is_never_consumed() {
    let fixture = tempfile::tempdir().expect("fixture");
    let root = Root::new(fixture.path()).expect("canonical root");
    let space = Space::new(root);
    space.domain_init("local", true).expect("initialize domain");
    space.task_start("local/refusal", true).expect("start task");
    let task = space.resolve("local/refusal").expect("resolve task");
    let memory = Memory::new(&task);
    memory.init("# Initial\n").expect("initialize memory");
    let held = memory.read().expect("held memory");

    let stale = fixture.path().join("STALE.md");
    std::fs::write(&stale, "# Stale\n").expect("write stale input");
    let output = run(
        fixture.path(),
        &[
            "--json",
            "memory",
            "write",
            "local/refusal",
            "--expect",
            &"0".repeat(64),
            "--file",
            stale.to_str().expect("stale path"),
        ],
    );
    assert!(!output.status.success());
    assert!(stale.is_file(), "failed CAS does not consume input");
    assert_eq!(
        memory.read().expect("unchanged memory").revision,
        held.revision
    );

    let output = run(
        fixture.path(),
        &[
            "--json",
            "memory",
            "write",
            "local/refusal",
            "--expect",
            &held.revision,
            "--file",
            memory.main().to_str().expect("managed path"),
        ],
    );
    assert!(!output.status.success());
    let error: Value = serde_json::from_slice(&output.stderr).expect("managed error");
    assert_eq!(error["error"]["code"], "memory.input_managed");
    assert!(memory.main().is_file(), "managed MAIN survives refusal");

    let directory = fixture.path().join("DIRECTORY");
    std::fs::create_dir(&directory).expect("directory input");
    let output = run(
        fixture.path(),
        &[
            "--json",
            "memory",
            "write",
            "local/refusal",
            "--expect",
            &held.revision,
            "--file",
            directory.to_str().expect("directory path"),
        ],
    );
    assert!(!output.status.success());
    let error: Value = serde_json::from_slice(&output.stderr).expect("type error");
    assert_eq!(error["error"]["code"], "memory.input_type");

    #[cfg(unix)]
    {
        let linked = fixture.path().join("LINKED.md");
        std::os::unix::fs::symlink(&stale, &linked).expect("linked input");
        let output = run(
            fixture.path(),
            &[
                "--json",
                "memory",
                "write",
                "local/refusal",
                "--expect",
                &held.revision,
                "--file",
                linked.to_str().expect("linked path"),
            ],
        );
        assert!(!output.status.success());
        let error: Value = serde_json::from_slice(&output.stderr).expect("symlink error");
        assert_eq!(error["error"]["code"], "memory.input_symlink");
        assert!(linked.is_symlink(), "refused symlink survives");
    }

    let duplicate = fixture.path().join("DUPLICATE.md");
    std::fs::write(&duplicate, "# Duplicate\n").expect("write duplicate");
    let output = run(
        fixture.path(),
        &[
            "--json",
            "memory",
            "settle",
            "local/refusal",
            "--expect",
            &held.revision,
            "--phase-file",
            duplicate.to_str().expect("duplicate path"),
            "--main-file",
            duplicate.to_str().expect("duplicate path"),
        ],
    );
    assert!(!output.status.success());
    let error: Value = serde_json::from_slice(&output.stderr).expect("duplicate error");
    assert_eq!(error["error"]["code"], "memory.input_duplicate");
    assert!(duplicate.is_file(), "duplicate refusal retains input");
}

#[cfg(target_os = "linux")]
#[test]
fn cleanup_failure_is_typed_and_does_not_roll_back_the_memory_write() {
    let fixture = tempfile::tempdir().expect("fixture");
    let root = Root::new(fixture.path()).expect("canonical root");
    let space = Space::new(root);
    space.domain_init("local", true).expect("initialize domain");
    space.task_start("local/cleanup", true).expect("start task");
    let task = space.resolve("local/cleanup").expect("resolve task");
    let memory = Memory::new(&task);
    memory.init("# Initial\n").expect("initialize memory");
    let held = memory.read().expect("held memory");

    let input = Path::new("/proc/version");
    let applied = std::fs::read_to_string(input).expect("read proc input");
    let output = run(
        fixture.path(),
        &[
            "--json",
            "memory",
            "write",
            "local/cleanup",
            "--expect",
            &held.revision,
            "--file",
            input.to_str().expect("input path"),
        ],
    );

    assert!(!output.status.success());
    let error: Value = serde_json::from_slice(&output.stderr).expect("typed error JSON");
    assert_eq!(error["error"]["code"], "memory.cleanup_after_apply");
    assert_eq!(error["error"]["details"]["applied"], true);
    assert_eq!(
        error["error"]["details"]["input_path"],
        input.display().to_string()
    );
    assert!(input.is_file(), "proc input remains");
    assert_eq!(memory.read().expect("applied memory").content, applied);
}
