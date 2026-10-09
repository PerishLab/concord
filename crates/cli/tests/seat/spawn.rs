use std::path::Path;
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
    let home = scratch.join("home");
    command
        .env("HOME", &home)
        .env("USERPROFILE", &home)
        .env("LOCALAPPDATA", home.join("data"))
        .env_remove("LOCUS_API")
        .env_remove("CLAUDE_CODE_SESSION_ID")
        .env_remove("GROK_SESSION_ID")
        .env_remove("CODEX_THREAD_ID")
        .env_remove("SANTI_SOUL_ID")
        .env_remove("SANTI_STRAND_ID");
    command
}
