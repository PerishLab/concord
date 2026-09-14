use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::Command;

pub(super) fn concord(scratch: &Path) -> Command {
    image(scratch, Path::new(env!("CARGO_BIN_EXE_concord")))
}

pub(super) fn image(scratch: &Path, binary: &Path) -> Command {
    let mut command = Command::new(binary);
    for (name, _) in std::env::vars() {
        if name.starts_with("CONCORD_") {
            command.env_remove(name);
        }
    }
    command
        .env("CONCORD_LOCUS_ENABLED", "false")
        .env("CONCORD_LOCUS_REPORT_FILE", scratch.join("stray.jsonl"))
        .env_remove("CLAUDE_CODE_SESSION_ID")
        .env_remove("GROK_SESSION_ID")
        .env_remove("CODEX_THREAD_ID");
    command
}

#[test]
fn sealed() {
    let tests = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests");
    let mut loose = Vec::new();
    sweep(&tests, &mut loose);
    assert!(
        loose.is_empty(),
        "spawn Concord through the seat, not directly: {loose:?}"
    );
}

fn sweep(root: &Path, loose: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(root).expect("tests") {
        let path = entry.expect("entry").path();
        if path.is_dir() && path.file_name() != Some(OsStr::new("seat")) {
            sweep(&path, loose);
        } else if loosed(&path) {
            loose.push(path);
        }
    }
}

fn loosed(path: &Path) -> bool {
    if path.extension() != Some(OsStr::new("rs")) {
        return false;
    }
    std::fs::read_to_string(path)
        .expect("source")
        .contains("CARGO_BIN_EXE_concord")
}
