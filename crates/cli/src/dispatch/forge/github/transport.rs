use crate::runtime::transport;
use std::ffi::OsString;
use std::path::Path;
pub use transport::{Failure, Reply, Stream};

pub struct Request<'a> {
    request: transport::Request<'a>,
    first: Option<OsString>,
}

impl<'a> Request<'a> {
    #[track_caller]
    pub fn new(command: &'a Path, timeout: u64, limit: usize) -> Self {
        Self {
            request: transport::Request::new(command, timeout, limit),
            first: None,
        }
    }

    pub fn arg(mut self, argument: impl Into<OsString>) -> Self {
        let argument = argument.into();
        if self.first.is_none() {
            self.first = Some(argument.clone());
        }
        self.request = self.request.arg(argument);
        self
    }

    pub fn args<I, S>(mut self, arguments: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<OsString>,
    {
        for argument in arguments {
            self = self.arg(argument);
        }
        self
    }

    pub fn input(mut self, input: &'a [u8]) -> Self {
        self.request = self.request.input(input);
        self
    }

    pub async fn run(mut self) -> Result<Reply, Failure> {
        if self.first.as_ref().is_some_and(|arg| arg == "api") {
            self.request = self.request.args(["--hostname", "github.com"]);
        }
        self.request.run().await
    }
}

pub(super) fn diagnostic(stderr: &[u8]) -> String {
    let text = String::from_utf8_lossy(stderr);
    if let Some(status) = http(&text) {
        return format!("reported HTTP status {status}");
    }
    for (needle, label) in [
        ("TLS handshake timeout", "TLS handshake timeout"),
        ("no such host", "DNS lookup failure"),
        ("connection refused", "connection refusal"),
        ("connection reset by peer", "connection reset"),
        ("context deadline exceeded", "request deadline exceeded"),
        ("unexpected EOF", "unexpected EOF"),
    ] {
        if text.contains(needle) {
            return format!("reported transport diagnostic: {label}");
        }
    }
    "no recognized provider diagnostic; cause unknown".into()
}

fn http(text: &str) -> Option<u16> {
    text.split("HTTP ").skip(1).find_map(|tail| {
        let code = tail.get(..3)?.parse::<u16>().ok()?;
        let boundary = tail
            .as_bytes()
            .get(3)
            .is_none_or(|byte| !byte.is_ascii_alphanumeric());
        ((100..=599).contains(&code) && boundary).then_some(code)
    })
}
