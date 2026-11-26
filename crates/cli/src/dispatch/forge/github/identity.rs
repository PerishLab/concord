use super::super::delivery::pull::{provider, stale};
use super::pull::{Commit, Pull, State};
use crate::runtime::github;
use concord_core::{Error, Kind, Reference, Result};
use runseal::github::{
    Client, Compare, Edit, Number, Proposal, Signal, Target, Verdict, lifecycle,
};

pub(super) struct Native<'a> {
    pub owner: &'a str,
    pub repository: &'a str,
    pub client: Client,
}

impl<'a> Native<'a> {
    pub(super) fn open(repository: &'a str, timeout: u64) -> Result<Self> {
        let (owner, repository) = coordinate(repository)?;
        let client = github::client(timeout).map_err(provider)?;
        Ok(Self {
            owner,
            repository,
            client,
        })
    }

    pub(super) fn listing(&self, base: &str, head: &str) -> Result<Vec<Pull>> {
        let mut result = Vec::new();
        let mut seen = std::collections::BTreeSet::new();
        let mut page = 1;
        for _ in 0..100 {
            let window = Number::new(page, 100).map_err(diagnostic)?;
            let input =
                lifecycle::Selection::new(head, Some(base), runseal::github::State::All, window)
                    .map_err(diagnostic)?;
            let slice = github::call("pull.listing", || {
                self.client.listing(self.owner, self.repository, &input)
            })
            .map_err(diagnostic)?
            .value;
            for item in slice.rows {
                if !seen.insert(item.number) {
                    return Err(provider("GitHub repeated a pull across pages"));
                }
                result.push(self.record(item)?);
            }
            match slice.next {
                Some(next) => page = next,
                None => return Ok(result),
            }
        }
        Err(provider(
            "GitHub pull listing did not complete within 100 pages",
        ))
    }

    pub(super) fn raise(&self, base: &str, head: &str, title: &str, body: &str) -> Result<Pull> {
        let scope = Compare::new(head, base).map_err(diagnostic)?;
        let proposal = Proposal::new(scope, title, Some(body)).map_err(diagnostic)?;
        let result = github::call("pull.propose", || {
            self.client.propose(self.owner, self.repository, &proposal)
        })
        .map_err(diagnostic)?;
        self.view(result.value.number.into())
    }

    pub(super) fn edit(&self, number: i64, title: &str, body: &str) -> Result<()> {
        let target = self.target(number)?;
        let change = Edit::new(title, body).map_err(diagnostic)?;
        github::call("pull.edit", || self.client.edit(&target, &change)).map_err(diagnostic)?;
        Ok(())
    }

    pub(super) fn view(&self, number: i64) -> Result<Pull> {
        let target = self.target(number)?;
        let result = github::call("pull.show", || self.client.pull(&target)).map_err(diagnostic)?;
        self.record(result.value)
    }

    fn record(&self, value: runseal::github::Pull) -> Result<Pull> {
        let record = receipt(value)?;
        pull(&record, &format!("{}/{}", self.owner, self.repository))?;
        Ok(record)
    }

    pub(super) fn mark(&self, candidate: &str, context: &str, description: &str) -> Result<()> {
        let signal =
            Signal::new(Verdict::Success, context, Some(description), None).map_err(diagnostic)?;
        github::call("pull.mark", || {
            self.client
                .mark(self.owner, self.repository, candidate, &signal)
        })
        .map_err(diagnostic)?;
        Ok(())
    }

    pub(super) fn checks(&self, number: i64) -> Result<super::pull::Checks> {
        use runseal::github::{Window, rollup::Rollup};
        let target = self.target(number)?;
        let head = github::call("pull.readiness", || self.client.readiness(&target))
            .map_err(diagnostic)?
            .value;
        let mut cursor = None;
        let mut total = None;
        let mut ids = std::collections::BTreeSet::new();
        let mut rows = Vec::new();
        for _ in 0..100 {
            let window = Window::new(100, cursor.as_deref()).map_err(diagnostic)?;
            let Rollup { pull, contexts, .. } = github::call("pull.rollup", || {
                self.client.rollup(&target, &head.head, &window)
            })
            .map_err(diagnostic)?
            .value;
            if pull != head.id {
                return Err(provider(
                    "GitHub pull identity moved while reading its checks",
                ));
            }
            let Some(page) = contexts else {
                return qualify(rows);
            };
            let current = (page.target, page.total);
            if total.get_or_insert(current.clone()) != &current {
                return Err(provider(
                    "GitHub check rollup identity or total changed during paging",
                ));
            }
            for row in page.rows {
                if !ids.insert(row.id().to_owned()) {
                    return Err(provider("GitHub repeated a check identity across pages"));
                }
                rows.push(row);
            }
            match page.next {
                Some(after) => cursor = Some(after),
                None if rows.len() == current.1 as usize => return qualify(rows),
                None => return Err(provider("GitHub check rollup is incomplete")),
            }
        }
        Err(provider(
            "GitHub check rollup did not complete within 100 pages",
        ))
    }

    pub(super) fn target(&self, number: i64) -> Result<Target> {
        let number = u32::try_from(number).map_err(|_| provider("invalid GitHub pull number"))?;
        Target::new(self.owner, self.repository, number).map_err(diagnostic)
    }
}

fn receipt(value: runseal::github::Pull) -> Result<Pull> {
    let (state, merge) = match (value.state.as_str(), value.landed.is_some(), value.merged) {
        ("open", false, None | Some(false)) => (State::Open, None),
        ("closed", false, None | Some(false)) => (State::Closed, None),
        ("closed", true, None | Some(true)) => {
            let oid = value
                .merge
                .ok_or_else(|| provider("merged GitHub pull lacks the merge commit"))?;
            (State::Merged, Some(Commit { oid }))
        }
        _ => return Err(provider("GitHub returned conflicting pull state")),
    };
    let updated = value
        .updated
        .ok_or_else(|| provider("GitHub pull lacks its update identity"))?;
    Ok(Pull {
        id: value.node,
        number: value.number.into(),
        url: value.url,
        state,
        base: value.base.branch,
        head: value.head.sha,
        merge,
        merged: value.landed,
        updated,
        title: value.title,
        body: value.body.unwrap_or_default(),
    })
}

pub(super) fn diagnostic(error: runseal::github::Error) -> concord_core::Error {
    provider(github::describe(error))
}

pub(crate) async fn repository(root: &std::path::Path) -> Result<String> {
    let remote =
        super::super::delivery::git::Git::run(root, &["remote", "get-url", "origin"]).await?;
    let path = remote
        .strip_prefix("git@github.com:")
        .or_else(|| remote.strip_prefix("ssh://git@github.com/"))
        .or_else(|| remote.strip_prefix("https://github.com/"))
        .ok_or_else(|| {
            concord_core::Error::typed(
                "concord.delivery.adaptor",
                "origin is not a supported github.com URL; select a qualified platform adaptor",
            )
        })?;
    let path = path.strip_suffix(".git").unwrap_or(path);
    let (owner, repository) = path
        .split_once('/')
        .filter(|(owner, repository)| {
            !owner.is_empty()
                && !repository.is_empty()
                && owner
                    .bytes()
                    .chain(repository.bytes())
                    .all(|byte| byte.is_ascii_alphanumeric() || b"-_.".contains(&byte))
        })
        .ok_or_else(|| stale("cannot derive GitHub owner/repository from origin"))?;
    Ok(format!("{owner}/{repository}"))
}

pub fn reference(repository: &str, number: i64) -> Result<Reference> {
    let (owner, repository) = coordinate(repository)?;
    Ok(Reference {
        kind: Kind::Change,
        provider: "github".to_string(),
        owner: owner.to_string(),
        repository: repository.to_string(),
        number,
    })
}

pub fn coordinate(repository: &str) -> Result<(&str, &str)> {
    repository.split_once('/').ok_or_else(|| {
        Error::typed(
            "concord.delivery.repository",
            "delivery repository coordinate is malformed",
        )
    })
}

pub(crate) fn validate(reference: &Reference, repository: &str) -> Result<()> {
    if reference.kind != Kind::Change
        || reference.provider != "github"
        || format!("{}/{}", reference.owner, reference.repository) != repository
    {
        return Err(stale(
            "recorded pull identity does not match the GitHub delivery repository",
        ));
    }
    Ok(())
}

pub(crate) fn pull(pull: &super::pull::Pull, repository: &str) -> Result<()> {
    if pull.id.is_empty()
        || pull.number < 1
        || pull.url != format!("https://github.com/{repository}/pull/{}", pull.number)
    {
        return Err(stale(
            "provider pull identity is outside the bound GitHub repository",
        ));
    }
    Ok(())
}

fn qualify(rows: Vec<runseal::github::rollup::Context>) -> Result<super::pull::Checks> {
    let rollup = serde_json::json!({"statusCheckRollup":rows});
    serde_json::from_value::<super::pull::Rollup>(rollup)
        .map_err(|_| provider("cannot encode GitHub check observations"))?
        .qualified()
}

#[cfg(test)]
mod tests {
    #[test]
    fn capabilities() {
        for rollup in [
            r#"{"statusCheckRollup":[{"__typename":"StatusContext","context":"gate","state":"UNKNOWN"}]}"#,
            r#"{"statusCheckRollup":[{"__typename":"CheckRun","name":"gate","status":"COMPLETED","conclusion":null}]}"#,
            r#"{"statusCheckRollup":[{"__typename":"CheckRun","name":"gate","status":"COMPLETED","conclusion":"UNSUPPORTED"}]}"#,
        ] {
            let decoded: super::super::pull::Rollup =
                serde_json::from_str(rollup).expect("wire rollup");
            assert_eq!(
                decoded.qualified().expect_err("unsupported checks").code(),
                "concord.delivery.adaptor"
            );
        }
    }
}
