use super::anchor::Coordinate;
use crate::{Error, Result};

impl Coordinate {
    pub fn parse(raw: &str) -> Result<Self> {
        let (seat, number) = raw.split_once('#').ok_or_else(coordinate)?;
        let (owner, repository) = seat.split_once('/').ok_or_else(coordinate)?;
        if owner.is_empty() || repository.is_empty() {
            return Err(coordinate());
        }
        if repository.contains('/') {
            return Err(coordinate());
        }
        if [owner, repository]
            .iter()
            .any(|part| part.chars().any(char::is_whitespace))
        {
            return Err(coordinate());
        }
        let number = number
            .parse::<i64>()
            .ok()
            .filter(|number| *number > 0)
            .ok_or_else(coordinate)?;
        Ok(Self {
            owner: owner.to_string(),
            repository: repository.to_string(),
            number,
        })
    }

    pub fn identity(&self) -> String {
        format!("{}/{}#{}", self.owner, self.repository, self.number)
    }

    pub(super) fn validate(&self) -> Result<()> {
        Self::parse(&self.identity()).map(|_| ())
    }
}

fn coordinate() -> Error {
    Error::typed(
        "concord.issue.coordinate",
        "Issue must be OWNER/REPOSITORY#NUMBER",
    )
}
