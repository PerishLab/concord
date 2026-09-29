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

pub(super) mod delivery {
    use super::{Authorities, keep};
    use crate::estate::{Anchor, Estate, Integration};
    use crate::{Error, Reference, Result, claim};
    use serde::Serialize;

    use super::super::delivery::{Plan, SCHEMA, agreement, member};
    use super::super::{IssueWorktree, issue_stale};

    #[derive(Clone, Debug, Eq, PartialEq, Serialize)]
    pub struct Resume {
        pub revision: i64,
        pub reference: Option<Reference>,
        pub released: bool,
    }

    struct Contract<'a> {
        plan: &'a Plan,
        snapshot: &'a plumb::delivery::Snapshot,
        anchor: &'a Anchor,
        integration: &'a Integration,
        estate: &'a Estate,
    }

    pub async fn resume<A: Authorities>(
        estate: &Estate,
        plan: &Plan,
        snapshot: &plumb::delivery::Snapshot,
    ) -> Result<Resume> {
        schema(plan)?;
        estate.ensure().await?;
        let anchor = estate.issue(&plan.issue).await?;
        let integration = estate.integration(&plan.issue).await?;
        Contract {
            plan,
            snapshot,
            anchor: &anchor,
            integration: &integration,
            estate,
        }
        .validate()?;
        keep::<A>(&plan.authority, "concord.delivery.authority")?;
        match estate.issue_member(&plan.issue).await {
            Ok(held) => active(estate, plan, &anchor, held).await,
            Err(error) if error.code() == "concord.member.absent" => released(plan, &anchor),
            Err(error) => Err(error),
        }
    }

    async fn active(
        estate: &Estate,
        plan: &Plan,
        anchor: &Anchor,
        held: IssueWorktree,
    ) -> Result<Resume> {
        let path = estate.issue_path(anchor)?;
        let source = estate.issue_source(&held)?;
        let proof = held.proof.as_ref().ok_or_else(|| {
            Error::typed(
                "concord.delivery.boundary",
                "Member has no current Boundary proof",
            )
        })?;
        agreement(&source, &path, &held, proof)?;
        if member(&held) != plan.member || proof != &plan.boundary {
            return Err(stale());
        }
        let references = estate.issue_references(held.key, anchor.key).await?;
        if references.len() > 1 {
            return Err(Error::typed(
                "concord.delivery.reference",
                "Member carries more than one pull coordinate",
            ));
        }
        let reference = references.into_iter().next();
        let replay = anchor.revision == plan.revision + 1 && reference.is_some();
        if anchor.revision != plan.revision && !replay {
            issue_stale(anchor.revision, plan.revision)?;
        }
        Ok(Resume {
            revision: anchor.revision,
            reference,
            released: false,
        })
    }

    fn released(plan: &Plan, anchor: &Anchor) -> Result<Resume> {
        if !matches!(anchor.revision - plan.revision, 1 | 2) {
            return Err(stale());
        }
        Ok(Resume {
            revision: anchor.revision,
            reference: None,
            released: true,
        })
    }

    fn schema(plan: &Plan) -> Result<()> {
        if plan.schema == SCHEMA && plan.delivery.schema == plumb::delivery::SCHEMA {
            return Ok(());
        }
        Err(Error::typed(
            "concord.delivery.schema",
            format!(
                "delivery plan schemas {} / {} are not {SCHEMA} / {}",
                plan.schema,
                plan.delivery.schema,
                plumb::delivery::SCHEMA
            ),
        ))
    }

    impl Contract<'_> {
        fn validate(&self) -> Result<()> {
            let repository = self.integration.repository.identity();
            let path = self.estate.issue_path(self.anchor)?;
            if self.plan.node != self.anchor.node || self.plan.issue != self.anchor.coordinate {
                return Err(stale());
            }
            if &self.plan.member.integration != self.integration {
                return Err(stale());
            }
            if self.plan.delivery.issue != *self.snapshot {
                return Err(stale());
            }
            if self.plan.delivery.repository != repository || self.plan.delivery.root != path {
                return Err(stale());
            }
            if self.plan.delivery.base != self.integration.branch {
                return Err(stale());
            }
            if self.plan.delivery.branch != self.plan.member.branch {
                return Err(stale());
            }
            if self.plan.delivery.source != self.plan.boundary.head {
                return Err(stale());
            }
            if self.plan.delivery.pull.title != self.snapshot.title {
                return Err(stale());
            }
            if self.plan.boundary.claim != claim::digest(&self.plan.member.claims) {
                return Err(stale());
            }
            Ok(())
        }
    }

    fn stale() -> Error {
        Error::typed(
            "concord.delivery.stale",
            "Issue, Integration, Member, Boundary, source, base, narrative, or released revision changed",
        )
    }
}
