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
