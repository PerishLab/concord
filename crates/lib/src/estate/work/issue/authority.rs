use crate::{Error, Result};
use plumb::guard::Authority;
use serde::{Deserialize, Serialize};
use std::path::Path;

type Found = std::result::Result<Authority, String>;

pub trait Authorities {
    fn compiled() -> Found;
    fn stable() -> Found;
}

pub struct Plumb;

impl Authorities for Plumb {
    fn compiled() -> Found {
        Authority::released()
    }

    fn stable() -> Found {
        Authority::stable()
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Warrant {
    pub producer: String,
    pub depot: String,
}

impl Warrant {
    fn of(authority: &Authority) -> Self {
        Self {
            producer: authority.producer().to_string(),
            depot: authority.depot().to_string(),
        }
    }

    fn names(&self, authority: &Authority) -> bool {
        self.producer == authority.producer() && self.depot == authority.depot()
    }
}

pub(super) fn select<A: Authorities>(
    root: &Path,
    commit: &str,
    code: &str,
) -> Result<(Authority, Warrant)> {
    let mut set = Vec::new();
    let mut absent = Vec::new();
    gather(A::compiled(), &mut set, &mut absent);
    let chosen = Authority::among(set.clone(), root, commit).or_else(|_| {
        gather(A::stable(), &mut set, &mut absent);
        Authority::among(set, root, commit)
            .map_err(|error| Error::typed(code, format!("{error}{}", unavailable(&absent))))
    })?;
    let warrant = Warrant::of(&chosen);
    Ok((chosen, warrant))
}

pub(super) fn keep<A: Authorities>(warrant: &Warrant, code: &str) -> Result<Authority> {
    let mut set = Vec::new();
    let mut absent = Vec::new();
    for found in [A::compiled as fn() -> Found, A::stable] {
        gather(found(), &mut set, &mut absent);
        if let Some(authority) = set.iter().find(|authority| warrant.names(authority)) {
            return Ok(authority.clone());
        }
    }
    let named = set
        .iter()
        .map(Authority::producer)
        .collect::<Vec<_>>()
        .join(", ");
    Err(Error::typed(
        code,
        format!(
            "plan was prepared under Plumb {} with depot {}, which is no longer an accepted Guard authority [{named}]{}; re-guard under the current Plumb and prepare again",
            warrant.producer,
            warrant.depot,
            unavailable(&absent)
        ),
    ))
}

fn gather(found: Found, set: &mut Vec<Authority>, absent: &mut Vec<String>) {
    match found {
        Ok(authority) => set.push(authority),
        Err(error) => absent.push(error),
    }
}

fn unavailable(absent: &[String]) -> String {
    absent
        .iter()
        .map(|error| format!("; unavailable authority: {error}"))
        .collect()
}
