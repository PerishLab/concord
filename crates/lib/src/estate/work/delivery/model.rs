use super::super::authority::{self, Authorities, Warrant};
use super::gate;
use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Mode {
    #[default]
    Plumb,
    WharfNative,
}

impl std::str::FromStr for Mode {
    type Err = &'static str;

    fn from_str(value: &str) -> std::result::Result<Self, Self::Err> {
        match value {
            "plumb" => Ok(Self::Plumb),
            "wharf-native" => Ok(Self::WharfNative),
            _ => Err("authority must be plumb or wharf-native"),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum Authority {
    Plumb {
        warrant: Warrant,
        guard: plumb::landing::Guard,
    },
    WharfNative {
        evidence: plumb::delivery::native::Evidence,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Candidate {
    pub root: PathBuf,
    pub repository: String,
    pub issue: plumb::delivery::Snapshot,
    pub observed: u64,
    pub base: String,
    pub target: String,
    pub branch: String,
    pub projection: String,
    pub source: String,
    pub candidate: String,
    pub pull: plumb::delivery::Narrative,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Preparation {
    pub root: PathBuf,
    pub base: String,
    pub target: String,
    pub branch: String,
    pub projection: String,
    pub source: String,
    pub candidate: String,
    pub title: String,
    pub body: String,
}

impl Authority {
    pub fn mode(&self) -> Mode {
        match self {
            Self::Plumb { .. } => Mode::Plumb,
            Self::WharfNative { .. } => Mode::WharfNative,
        }
    }

    pub(in crate::estate::work::issue) fn validate<A: Authorities>(
        &self,
        integration: &crate::estate::Integration,
    ) -> Result<()> {
        match self {
            Self::Plumb { warrant, .. } => {
                authority::keep::<A>(warrant, "concord.delivery.authority")?;
            }
            Self::WharfNative { evidence } => {
                admit(integration)?;
                if evidence.authority != gate::AUTHORITY {
                    return Err(Error::typed(
                        "concord.delivery.authority",
                        "native gate implementation changed; prepare again",
                    ));
                }
            }
        }
        Ok(())
    }
}

pub(super) fn admit(integration: &crate::estate::Integration) -> Result<()> {
    if integration.node == "R_kgDOUesM0Q"
        && integration.repository.owner == "PerishLab"
        && integration.repository.name == "wharf"
    {
        return Ok(());
    }
    Err(Error::typed(
        "concord.delivery.authority",
        "Wharf-native delivery requires the registered PerishLab/wharf repository node",
    ))
}

impl Candidate {
    pub fn preparation(&self) -> Preparation {
        Preparation {
            root: self.root.clone(),
            base: self.base.clone(),
            target: self.target.clone(),
            branch: self.branch.clone(),
            projection: self.projection.clone(),
            source: self.source.clone(),
            candidate: self.candidate.clone(),
            title: self.pull.title.clone(),
            body: self.pull.body.clone(),
        }
    }

    pub(super) fn guard(&self, guard: &plumb::landing::Guard) -> plumb::delivery::Plan {
        plumb::delivery::Plan {
            schema: plumb::delivery::SCHEMA.into(),
            root: self.root.clone(),
            repository: self.repository.clone(),
            issue: self.issue.clone(),
            observed: self.observed,
            base: self.base.clone(),
            target: self.target.clone(),
            branch: self.branch.clone(),
            projection: self.projection.clone(),
            source: self.source.clone(),
            candidate: self.candidate.clone(),
            pull: self.pull.clone(),
            guard: guard.clone(),
        }
    }

    pub(super) fn native(
        &self,
        evidence: &plumb::delivery::native::Evidence,
    ) -> plumb::delivery::native::Plan {
        plumb::delivery::native::Plan {
            schema: plumb::delivery::native::SCHEMA.into(),
            root: self.root.clone(),
            repository: self.repository.clone(),
            issue: self.issue.clone(),
            observed: self.observed,
            base: self.base.clone(),
            target: self.target.clone(),
            branch: self.branch.clone(),
            projection: self.projection.clone(),
            source: self.source.clone(),
            candidate: self.candidate.clone(),
            pull: self.pull.clone(),
            evidence: evidence.clone(),
        }
    }
}

impl From<plumb::delivery::Plan> for Candidate {
    fn from(plan: plumb::delivery::Plan) -> Self {
        Self {
            root: plan.root,
            repository: plan.repository,
            issue: plan.issue,
            observed: plan.observed,
            base: plan.base,
            target: plan.target,
            branch: plan.branch,
            projection: plan.projection,
            source: plan.source,
            candidate: plan.candidate,
            pull: plan.pull,
        }
    }
}

impl From<plumb::delivery::native::Plan> for Candidate {
    fn from(plan: plumb::delivery::native::Plan) -> Self {
        Self {
            root: plan.root,
            repository: plan.repository,
            issue: plan.issue,
            observed: plan.observed,
            base: plan.base,
            target: plan.target,
            branch: plan.branch,
            projection: plan.projection,
            source: plan.source,
            candidate: plan.candidate,
            pull: plan.pull,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::admit;
    use crate::estate::{Integration, Repository};

    #[test]
    fn identity() {
        let wharf = Integration {
            key: 1,
            node: "R_kgDOUesM0Q".into(),
            repository: Repository {
                owner: "PerishLab".into(),
                name: "wharf".into(),
            },
            path: "/tmp/wharf".into(),
            common: "/tmp/wharf/.git".into(),
            remote: "origin".into(),
            branch: "main".into(),
        };
        admit(&wharf).expect("exact identity");
        for other in [
            Integration {
                node: "R_other".into(),
                ..wharf.clone()
            },
            Integration {
                repository: Repository {
                    owner: "other".into(),
                    ..wharf.repository.clone()
                },
                ..wharf.clone()
            },
            Integration {
                repository: Repository {
                    name: "concord".into(),
                    ..wharf.repository.clone()
                },
                ..wharf.clone()
            },
        ] {
            assert_eq!(
                admit(&other).expect_err("other identity").code(),
                "concord.delivery.authority"
            );
        }
    }
}
