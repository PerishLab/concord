use super::{Integration, guard, registration};
use crate::estate::{Estate, fault};
use crate::{Error, Result};
use plumb::integration::Relation;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

const SCHEMA: &str = "concord.integration-rebind/v1";

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Rebind {
    pub schema: String,
    pub space: String,
    pub integration: Integration,
    pub path: PathBuf,
    pub common: PathBuf,
    pub head: String,
}

impl Estate {
    pub async fn relocation(&self, expected: &Integration, path: &Path) -> Result<Rebind> {
        let _integration = guard(self, expected)?;
        let _space = self.guard()?;
        self.rebinding(expected).await?;
        let target = self.destination(expected, path).await?;
        Ok(Rebind {
            schema: SCHEMA.into(),
            space: self.identity().into(),
            integration: expected.clone(),
            path: target.path,
            common: target.common,
            head: target.head,
        })
    }

    pub async fn rebind(&self, plan: &Rebind, apply: bool) -> Result<Integration> {
        if plan.schema != SCHEMA {
            return Err(refused("unsupported rebind schema; prepare a fresh plan"));
        }
        self.identify(&plan.space)?;
        let _integration = guard(self, &plan.integration)?;
        let _space = self.guard()?;
        self.rebinding(&plan.integration).await?;
        let target = self.destination(&plan.integration, &plan.path).await?;
        if target.path != plan.path || target.common != plan.common || target.head != plan.head {
            return Err(refused(
                "target checkout changed after preparation; preserve it and prepare again",
            ));
        }
        absent(&plan.integration.path)?;
        let path = target.path.display().to_string();
        let common = target.common.display().to_string();
        if apply {
            self.core
                .batch(async |tx| {
                    tx.set(
                        "Integration",
                        plan.integration.key,
                        &[("path", path.as_str()), ("common", common.as_str())],
                    )
                    .await?;
                    Ok(())
                })
                .await
                .map_err(fault)?;
        }
        Ok(Integration {
            path,
            common,
            ..plan.integration.clone()
        })
    }

    async fn rebinding(&self, expected: &Integration) -> Result<()> {
        if self.locate(&expected.node).await? != *expected {
            return Err(refused(
                "Integration changed; reread its exact key, node and registration",
            ));
        }
        if expected.remote != "origin" || expected.branch != "main" {
            return Err(refused(
                "original Integration has an invalid remote or branch",
            ));
        }
        if self
            .core
            .live("IssueMember")
            .await
            .map_err(fault)?
            .iter()
            .any(|row| row.int("integration") == Some(expected.key))
        {
            return Err(Error::typed(
                "concord.integration.dependent",
                "rebind requires an idle Integration; settle live Members before moving its checkout",
            ));
        }
        absent(&expected.path)?;
        let mut report = self.inspect().await?;
        report.faults.retain(|finding| {
            finding.code != "integration.agreement"
                || finding.subject != expected.repository.identity()
        });
        if !report.agrees() {
            return Err(Error::detailed(
                "concord.audit.refused",
                "Integration rebind refuses unrelated agreement faults",
                serde_json::json!({"agreement": report}),
            ));
        }
        Ok(())
    }

    async fn destination(&self, expected: &Integration, path: &Path) -> Result<Destination> {
        if registration::origin(path)? != expected.repository {
            return Err(refused(
                "target origin does not match the registered repository",
            ));
        }
        let observed = registration::observe(path)?;
        if observed.relation != Relation::Equal {
            return Err(refused("target main must equal its fetched origin/main"));
        }
        let worktrees = observed
            .worktrees
            .iter()
            .filter(|worktree| !worktree.staging)
            .collect::<Vec<_>>();
        if worktrees.len() != 1
            || worktrees[0].path.canonicalize().ok().as_ref() != Some(&observed.checkout.path)
        {
            return Err(refused("target requires no auxiliary Git worktrees"));
        }
        if observed.checkout.path.starts_with(&self.layout.root) {
            return Err(refused(
                "Integration checkout must be outside private Space execution seats",
            ));
        }
        let target = Destination {
            path: observed.checkout.path,
            common: observed.checkout.common,
            head: observed.checkout.head,
        };
        for held in self.integrations().await? {
            if held.key != expected.key
                && (Path::new(&held.path) == target.path
                    || Path::new(&held.common) == target.common)
            {
                return Err(refused(
                    "target checkout or Git common directory is already registered",
                ));
            }
        }
        Ok(target)
    }
}

struct Destination {
    path: PathBuf,
    common: PathBuf,
    head: String,
}

fn absent(path: &str) -> Result<()> {
    match std::fs::symlink_metadata(path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
        Ok(_) => Err(refused(
            "original checkout still exists; rebind only an explicitly relocated idle Integration, without deleting or overwriting payload",
        )),
    }
}

fn refused(message: impl Into<String>) -> Error {
    Error::typed("concord.integration.rebind", message)
}
