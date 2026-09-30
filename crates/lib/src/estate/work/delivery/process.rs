use plumb::landing::Refusal;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

const LIMIT: u64 = 1_048_576;
const TIMEOUT: Duration = Duration::from_secs(300);

struct Running(Child);

impl Drop for Running {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

pub(super) fn run(command: &mut Command) -> Result<String, Refusal> {
    bounded(command, TIMEOUT)
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
        if log.metadata().map_err(fault)?.len() > LIMIT || started.elapsed() > timeout {
            return Err(refuse("native gate exceeded its time or output budget"));
        }
        if let Some(status) = child.0.try_wait().map_err(fault)? {
            log.seek(SeekFrom::Start(0)).map_err(fault)?;
            let mut output = String::new();
            log.take(LIMIT + 1)
                .read_to_string(&mut output)
                .map_err(fault)?;
            if output.len() as u64 > LIMIT {
                return Err(refuse("native gate exceeded its output budget"));
            }
            if !status.success() {
                return Err(refuse(format!("native gate failed ({status}): {output}")));
            }
            return Ok(output);
        }
        std::thread::sleep(Duration::from_millis(25));
    }
}

pub(super) fn hash(path: &Path) -> Result<String, Refusal> {
    use sha2::{Digest, Sha256};
    let mut file = std::fs::File::open(path).map_err(fault)?;
    let mut digest = Sha256::new();
    let mut buffer = [0_u8; 65536];
    loop {
        let size = file.read(&mut buffer).map_err(fault)?;
        if size == 0 {
            return Ok(format!("{:x}", digest.finalize()));
        }
        digest.update(&buffer[..size]);
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
    use super::{Duration, Instant, bounded};
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
                .contains("budget")
        );
        let mut slow = Command::new(&python);
        slow.env_clear()
            .args(["-B", "-c", "import time; time.sleep(2)"]);
        let before = Instant::now();
        assert!(
            bounded(&mut slow, Duration::from_millis(10))
                .expect_err("timeout")
                .message
                .contains("budget")
        );
        assert!(before.elapsed() < Duration::from_secs(1));
    }
}
