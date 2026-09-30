use std::ffi::OsString;
use std::fmt::{Display, Formatter};
use std::path::Path;
use std::process::{ExitStatus, Stdio};
use std::time::Duration;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWriteExt};
use tokio::process::Command;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Stream {
    Stdout,
    Stderr,
}

#[derive(Debug)]
pub enum Failure {
    Spawn(std::io::Error),
    Exchange(std::io::Error),
    Timeout,
    Oversized(Stream),
    Refused { status: ExitStatus, stderr: Vec<u8> },
}

pub struct Reply {
    pub stdout: Vec<u8>,
}

struct Completion {
    stdout: Vec<u8>,
    stderr: Vec<u8>,
    status: ExitStatus,
}

pub struct Request<'a> {
    command: &'a Path,
    arguments: Vec<OsString>,
    input: Option<&'a [u8]>,
    timeout: Duration,
    limit: usize,
}

impl<'a> Request<'a> {
    pub fn new(command: &'a Path, timeout: u64, limit: usize) -> Self {
        Self {
            command,
            arguments: Vec::new(),
            input: None,
            timeout: Duration::from_secs(timeout),
            limit,
        }
    }

    pub fn arg(mut self, argument: impl Into<OsString>) -> Self {
        self.arguments.push(argument.into());
        self
    }

    pub fn args<I, S>(mut self, arguments: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<OsString>,
    {
        self.arguments.extend(arguments.into_iter().map(Into::into));
        self
    }

    pub fn input(mut self, input: &'a [u8]) -> Self {
        self.input = Some(input);
        self
    }

    pub async fn run(self) -> Result<Reply, Failure> {
        let mut process = Command::new(self.command);
        process
            .args(self.arguments)
            .stdin(if self.input.is_some() {
                Stdio::piped()
            } else {
                Stdio::null()
            })
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        let mut child = process.spawn().map_err(Failure::Spawn)?;
        let mut stdin = child.stdin.take();
        let stdout = child.stdout.take().expect("piped GitHub stdout");
        let stderr = child.stderr.take().expect("piped GitHub stderr");
        let exchange = async {
            let input = async {
                if let (Some(input), Some(mut stdin)) = (self.input, stdin.take()) {
                    stdin.write_all(input).await.map_err(Failure::Exchange)?;
                    stdin.shutdown().await.map_err(Failure::Exchange)?;
                }
                Ok(())
            };
            let wait = async { child.wait().await.map_err(Failure::Exchange) };
            let replies = async {
                let (stdout, stderr, status) = tokio::try_join!(
                    bounded(stdout, self.limit, Stream::Stdout),
                    bounded(stderr, self.limit, Stream::Stderr),
                    wait,
                )?;
                Ok::<_, Failure>(Completion {
                    stdout,
                    stderr,
                    status,
                })
            };
            let ((), completion) = tokio::try_join!(input, replies)?;
            Ok::<_, Failure>(completion)
        };
        let completion = tokio::time::timeout(self.timeout, exchange)
            .await
            .map_err(|_| Failure::Timeout)??;
        if !completion.status.success() {
            return Err(Failure::Refused {
                status: completion.status,
                stderr: completion.stderr,
            });
        }
        Ok(Reply {
            stdout: completion.stdout,
        })
    }
}

async fn bounded(
    reader: impl AsyncRead + Unpin,
    limit: usize,
    stream: Stream,
) -> Result<Vec<u8>, Failure> {
    let mut bytes = Vec::new();
    reader
        .take((limit + 1) as u64)
        .read_to_end(&mut bytes)
        .await
        .map_err(Failure::Exchange)?;
    if bytes.len() > limit {
        return Err(Failure::Oversized(stream));
    }
    Ok(bytes)
}

impl Display for Failure {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Spawn(error) => write!(formatter, "cannot run GitHub command: {error}"),
            Self::Exchange(error) => write!(formatter, "GitHub command exchange failed: {error}"),
            Self::Timeout => formatter.write_str("GitHub command timed out"),
            Self::Oversized(stream) => {
                write!(formatter, "GitHub {stream:?} exceeds the reply limit")
            }
            Self::Refused { status, stderr } => write!(
                formatter,
                "GitHub command was refused with {status}: {}",
                String::from_utf8_lossy(stderr).trim()
            ),
        }
    }
}
