use super::{BRANCH, Integration, REMOTE, Register, Repository, TRACKING, node};
use crate::estate::{Estate, fault};
use crate::{Error, Result, git};
use plumb::integration::{Expectation, Relation};
use std::path::Path;

impl Estate {
    pub async fn register(&self, request: &Register) -> Result<Integration> {
        node(&request.node)?;
        request.repository.validate()?;
        let _guard = self.guard()?;
        self.ensure().await?;
        let remote = origin(&request.path)?;
        if remote != request.repository {
            return Err(Error::typed(
                "concord.integration.repository",
                format!(
                    "origin repository {} does not match {}",
                    remote.identity(),
                    request.repository.identity()
                ),
            ));
        }
        let inspection = observe(&request.path)?;
        if inspection.relation != Relation::Equal {
            return Err(Error::typed(
                "concord.integration.refused",
                "integration registration requires main to equal its fetched origin/main",
            ));
        }
        if inspection.worktrees.len() != 1
            || inspection.worktrees[0].path.canonicalize().ok().as_ref()
                != Some(&inspection.checkout.path)
        {
            return Err(Error::typed(
                "concord.integration.worktrees",
                "integration registration requires no auxiliary Git worktrees",
            ));
        }
        let path = inspection.checkout.path.display().to_string();
        let common = inspection.checkout.common.display().to_string();
        for integration in self.integrations().await? {
            reserved(&integration, request, &path, &common)?;
        }
        let key = self
            .core
            .put(
                "Integration",
                &[
                    ("node", request.node.as_str()),
                    ("owner", request.repository.owner.as_str()),
                    ("repository", request.repository.name.as_str()),
                    ("path", path.as_str()),
                    ("common", common.as_str()),
                    ("remote", REMOTE),
                    ("branch", BRANCH),
                ],
            )
            .await
            .map_err(fault)?;
        Ok(Integration {
            key,
            node: request.node.clone(),
            repository: request.repository.clone(),
            path,
            common,
            remote: REMOTE.to_string(),
            branch: BRANCH.to_string(),
        })
    }
}

pub(in crate::estate) fn observe(path: &Path) -> Result<plumb::integration::Inspection> {
    let target = git::at(path).text(&["rev-parse", &format!("{TRACKING}^{{commit}}")])?;
    let expected = Expectation::new(BRANCH, TRACKING, target);
    let inspection = plumb::integration::inspect(path, &expected)
        .map_err(|error| Error::typed("concord.integration.refused", error.to_string()))?;
    if matches!(inspection.relation, Relation::Ahead | Relation::Diverged) {
        return Err(unhealthy());
    }
    if !inspection.checkout.clean {
        return Err(unhealthy());
    }
    if inspection.checkout.branch.as_deref() != Some(BRANCH) {
        return Err(unhealthy());
    }
    if inspection.checkout.upstream.as_deref() != Some(TRACKING) {
        return Err(unhealthy());
    }
    Ok(inspection)
}

fn reserved(integration: &Integration, request: &Register, path: &str, common: &str) -> Result<()> {
    if integration.node == request.node {
        return Err(Error::typed(
            "concord.integration.node_reserved",
            format!("repository node is already registered: {}", request.node),
        ));
    }
    if integration.repository == request.repository {
        return Err(Error::typed(
            "concord.integration.coordinate_reserved",
            format!(
                "repository coordinate is already registered: {}",
                request.repository.identity()
            ),
        ));
    }
    if integration.path == path || integration.common == common {
        return Err(Error::typed(
            "concord.integration.git_reserved",
            "integration checkout or Git common directory is already registered",
        ));
    }
    Ok(())
}

fn origin(path: &Path) -> Result<Repository> {
    let value = git::at(path).text(&["remote", "get-url", REMOTE])?;
    let coordinate = value
        .strip_prefix("https://github.com/")
        .or_else(|| value.strip_prefix("http://github.com/"))
        .or_else(|| value.strip_prefix("git@github.com:"))
        .or_else(|| value.strip_prefix("ssh://git@github.com/"))
        .map(|value| value.strip_suffix(".git").unwrap_or(value))
        .ok_or_else(|| {
            Error::typed(
                "concord.integration.repository",
                "origin must identify one GitHub OWNER/REPOSITORY",
            )
        })?;
    Repository::parse(coordinate).map_err(|_| {
        Error::typed(
            "concord.integration.repository",
            "origin must identify one GitHub OWNER/REPOSITORY",
        )
    })
}

fn unhealthy() -> Error {
    Error::typed(
        "concord.integration.refused",
        "integration checkout must be clean on main, track origin/main, and have no local-only commit",
    )
}
