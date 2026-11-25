use super::fault;
use crate::runtime::github;
use concord_core::Result;

pub(super) struct Client {
    pub timeout: u64,
}

impl Client {
    pub fn open(&self) -> Result<runseal::github::Client> {
        github::client(self.timeout).map_err(|error| fault("unavailable", error))
    }
}

pub(super) fn provider(error: runseal::github::Error) -> concord_core::Error {
    fault("unavailable", github::describe(error))
}
