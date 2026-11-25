use runseal::github::{Client, Error, Outcome, Receipt};
use std::net::SocketAddr;
use std::panic::Location;
use std::path::Path;
use std::time::Instant;

pub(crate) fn client(timeout: u64) -> Result<Client, String> {
    let held = match plumb::config::value("CONCORD_GITHUB_LOOPBACK") {
        Some(address) => {
            let address: SocketAddr = address.parse().map_err(|_| {
                "CONCORD_GITHUB_LOOPBACK must be an IP:PORT loopback address".to_string()
            })?;
            Client::loopback(&token()?, address)
        }
        None => match plumb::config::value("GITHUB_TOKEN_FILE") {
            Some(path) => Client::open(Path::new(&path)),
            None => Client::token(&token()?),
        },
    };
    held.map_err(describe)?.timeout(timeout).map_err(describe)
}

#[cfg(test)]
pub(crate) fn loopback(address: SocketAddr, timeout: u64) -> Result<Client, String> {
    Client::loopback("fixture_token", address)
        .map_err(describe)?
        .timeout(timeout)
        .map_err(describe)
}

#[track_caller]
pub(crate) fn call<T>(
    operation: &str,
    run: impl FnOnce() -> Result<Receipt<T>, Error>,
) -> Result<Receipt<T>, Error> {
    let site = Location::caller();
    let place = format!("{}:{}", site.file(), site.line());
    let start = Instant::now();
    let reply = run();
    let outcome = match &reply {
        Ok(_) => "ok",
        Err(error) => match error.outcome {
            Outcome::Unsent => "unsent",
            Outcome::Refused => "refused",
            Outcome::Unknown => "unknown",
            _ => "error",
        },
    };
    crate::observation::call(operation, &place, start.elapsed(), outcome);
    reply
}

pub(crate) fn describe(error: Error) -> String {
    format!(
        "{error}; HTTP status {:?}; request {:?}",
        error.evidence.status, error.evidence.request
    )
}

fn token() -> Result<String, String> {
    plumb::config::value("GH_TOKEN")
        .or_else(|| plumb::config::value("GITHUB_TOKEN"))
        .ok_or_else(|| "select a Runseal profile carrying a GitHub token".to_string())
}
