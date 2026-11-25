use plumb::landing::Refusal;
use std::io::{Read, Seek, SeekFrom};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

const LIMIT: u64 = 1_048_576;
const TIMEOUT: Duration = Duration::from_secs(300);
const SUITE: Duration = Duration::from_secs(1800);

struct Running(Child);

impl Drop for Running {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

#[locus::trace(with = plumb::observation::view())]
pub(super) fn run(command: &mut Command) -> Result<String, Refusal> {
    bounded(command, TIMEOUT)
}

#[locus::trace(with = plumb::observation::view())]
pub(super) fn suite(command: &mut Command) -> Result<String, Refusal> {
    bounded(command, SUITE)
}

fn bounded(command: &mut Command, timeout: Duration) -> Result<String, Refusal> {
    let mut log = tempfile::tempfile().map_err(fault)?;
    command
        .stdin(Stdio::null())
        .stdout(log.try_clone().map_err(fault)?)
        .stderr(log.try_clone().map_err(fault)?);
    let mut child = Running(command.spawn().map_err(fault)?);
    let started = Instant::now();
    loop {
        if log.metadata().map_err(fault)?.len() > LIMIT {
            return Err(refuse("native gate exceeded its output budget"));
        }
        if started.elapsed() > timeout {
            return Err(refuse("native gate exceeded its time budget"));
        }
        if let Some(status) = child.0.try_wait().map_err(fault)? {
            log.seek(SeekFrom::Start(0)).map_err(fault)?;
            let mut bytes = Vec::new();
            log.take(LIMIT + 1).read_to_end(&mut bytes).map_err(fault)?;
            if bytes.len() as u64 > LIMIT {
                return Err(refuse("native gate exceeded its output budget"));
            }
            let output = String::from_utf8(bytes)
                .map_err(|error| refuse(format!("native gate output is not UTF-8: {error}")))?;
            if !status.success() {
                return Err(refuse(format!("native gate failed ({status}): {output}")));
            }
            return Ok(output);
        }
        std::thread::sleep(Duration::from_millis(25));
    }
}

pub(super) fn refuse(message: impl Into<String>) -> Refusal {
    Refusal {
        kind: "native",
        message: message.into(),
    }
}

fn fault(error: std::io::Error) -> Refusal {
    refuse(format!("native gate process: {error}"))
}

#[cfg(all(test, unix))]
mod tests {
    use super::{Duration, Instant, LIMIT, SUITE, TIMEOUT, bounded, run, suite};
    use std::process::Command;

    #[test]
    fn budget() {
        let python = which::which("python3").expect("Python");
        let mut large = Command::new(&python);
        large.env_clear().args(["-B", "-c", "print('x' * 2097152)"]);
        assert!(
            bounded(&mut large, Duration::from_secs(5))
                .expect_err("large output")
                .message
                .contains("output budget")
        );
        let mut slow = Command::new(&python);
        slow.env_clear()
            .args(["-B", "-c", "import time; time.sleep(2)"]);
        let before = Instant::now();
        assert!(
            bounded(&mut slow, Duration::from_millis(10))
                .expect_err("timeout")
                .message
                .contains("time budget")
        );
        assert!(before.elapsed() < Duration::from_secs(1));
    }

    #[test]
    fn policy() {
        assert_eq!(TIMEOUT, Duration::from_secs(300));
        assert_eq!(SUITE, Duration::from_secs(1800));
        assert_eq!(LIMIT, 1_048_576);
        let python = which::which("python3").expect("Python");
        let mut command = Command::new(&python);
        command.env_clear().args(["-B", "-c", "print('verified')"]);
        assert_eq!(suite(&mut command).expect("suite"), "verified\n");
        let mut command = Command::new(&python);
        command.env_clear().args(["--version"]);
        assert!(run(&mut command).expect("observation").contains("Python"));
    }

    #[test]
    fn failure() {
        let python = which::which("python3").expect("Python");
        let mut command = Command::new(&python);
        command
            .env_clear()
            .args(["-B", "-c", "raise SystemExit(7)"]);
        assert!(
            suite(&mut command)
                .expect_err("exit")
                .message
                .contains("failed")
        );
    }

    #[test]
    fn window() {
        let python = which::which("python3").expect("Python");
        let mut command = Command::new(&python);
        command
            .env_clear()
            .args(["-B", "-c", "import time; time.sleep(0.1)"]);
        assert!(bounded(&mut command, Duration::from_secs(2)).is_ok());
        let mut command = Command::new(&python);
        command
            .env_clear()
            .args(["-B", "-c", "import time; time.sleep(2)"]);
        assert!(bounded(&mut command, Duration::from_millis(10)).is_err());
    }
}

#[cfg(all(test, windows))]
mod windows {
    use super::{Duration, Instant, LIMIT, SUITE, TIMEOUT, bounded, run, suite};
    use radicula_process::Tool;
    use std::path::{Path, PathBuf};
    use std::process::Command;

    fn python() -> PathBuf {
        let search = crate::config::tools().expect("search");
        search
            .resolve("python3")
            .and_then(Tool::at)
            .or_else(|_| search.resolve("python").and_then(Tool::at))
            .expect("python")
            .path
    }

    fn command(program: &Path) -> Command {
        let mut command = Command::new(program);
        command
            .env_clear()
            .env("PYTHONUTF8", "1")
            .env("PYTHONIOENCODING", "utf-8");
        for name in ["SystemRoot", "SYSTEMROOT", "WINDIR", "TEMP", "TMP"] {
            if let Some(value) = crate::config::variable(name) {
                command.env(name, value);
            }
        }
        command
    }

    #[test]
    fn budget() {
        let program = python();
        let mut large = command(&program);
        large.args(["-B", "-c", "print('x' * 2097152)"]);
        assert!(
            bounded(&mut large, Duration::from_secs(10))
                .expect_err("large output")
                .message
                .contains("output budget")
        );
        let mut slow = command(&program);
        slow.args(["-B", "-c", "import time; time.sleep(2)"]);
        let before = Instant::now();
        assert!(
            bounded(&mut slow, Duration::from_millis(10))
                .expect_err("timeout")
                .message
                .contains("time budget")
        );
        assert!(before.elapsed() < Duration::from_secs(1));
    }

    #[test]
    fn capture() {
        assert_eq!(TIMEOUT, Duration::from_secs(300));
        assert_eq!(SUITE, Duration::from_secs(1800));
        assert_eq!(LIMIT, 1_048_576);
        let program = python();
        let mut utf8 = command(&program);
        utf8.args(["-B", "-c", "print('verified ✓ 中文')"]);
        assert_eq!(suite(&mut utf8).expect("utf8").trim(), "verified ✓ 中文");
        let mut version = command(&program);
        version.args(["--version"]);
        assert!(
            run(&mut version)
                .expect("version")
                .trim()
                .starts_with("Python 3.")
        );
        let mut failed = command(&program);
        failed.args(["-B", "-c", "raise SystemExit(7)"]);
        assert!(
            suite(&mut failed)
                .expect_err("exit")
                .message
                .contains("failed")
        );
    }
}
