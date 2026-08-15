use super::super::{Estate, World, fault};
use crate::component;
use crate::{Error, Result};
use keel::Tx;
use keel::adapt::db::Sqlite;
use serde::Serialize;

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Repository {
    pub key: i64,
    pub domain: String,
    pub name: String,
    pub note: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Annotate {
    pub domain: String,
    pub name: String,
    pub note: Option<String>,
    pub revision: i64,
}

impl Estate {
    pub async fn repositories(&self, domain: Option<&str>) -> Result<Vec<Repository>> {
        let world = World::load(self).await?;
        if let Some(domain) = domain
            && world.domain(domain).is_none()
        {
            return Err(world.absent(domain));
        }
        let mut found = Vec::new();
        for row in self.core.live("Repository").await.map_err(fault)? {
            let root = row.int("domain").ok_or_else(|| malformed(row.key()))?;
            let realm = world
                .domains
                .get(&root)
                .ok_or_else(|| malformed(row.key()))?;
            if domain.is_some_and(|wanted| wanted != realm.name) {
                continue;
            }
            found.push(Repository {
                key: row.key(),
                domain: realm.name.clone(),
                name: row
                    .text("name")
                    .ok_or_else(|| malformed(row.key()))?
                    .to_string(),
                note: row.text("note").map(str::to_string),
            });
        }
        found.sort_by_key(|repo| (repo.domain.clone(), repo.name.clone(), repo.key));
        Ok(found)
    }

    pub async fn annotate(&self, request: &Annotate) -> Result<(Repository, i64)> {
        component("domain name", &request.domain)?;
        component("repository name", &request.name)?;
        if request
            .note
            .as_ref()
            .is_some_and(|note| note.trim().is_empty())
        {
            return Err(Error::typed(
                "concord.repository.note",
                "Repository note cannot be blank",
            ));
        }
        let _guard = self.guard()?;
        self.ensure().await?;
        let world = World::load(self).await?;
        let domain = world.domain(&request.domain).ok_or_else(|| {
            Error::typed(
                "concord.domain.absent",
                format!("unknown managed domain {}", request.domain),
            )
        })?;
        let realm = &world.domains[&domain];
        if realm.revision != request.revision {
            return Err(Error::typed(
                "concord.domain.stale",
                format!(
                    "Domain revision changed: expected {}, found {}",
                    request.revision, realm.revision
                ),
            ));
        }
        let current = self
            .repositories(Some(&request.domain))
            .await?
            .into_iter()
            .find(|repo| repo.name == request.name);
        let revision = realm.revision + 1;
        let next = revision.to_string();
        let key = self
            .core
            .batch(async |tx| {
                let key = write(tx, request, &current, domain).await?;
                tx.set("Domain", domain, &[("revision", next.as_str())])
                    .await?;
                Ok(key)
            })
            .await
            .map_err(fault)?;
        Ok((
            Repository {
                key,
                domain: request.domain.clone(),
                name: request.name.clone(),
                note: request.note.clone(),
            },
            revision,
        ))
    }
}

async fn write(
    tx: &mut Tx<'_, Sqlite>,
    request: &Annotate,
    current: &Option<Repository>,
    domain: i64,
) -> std::result::Result<i64, keel::adapt::Error> {
    match current {
        Some(repo) => {
            let note = request.note.as_deref().unwrap_or("");
            if note.is_empty() {
                tx.unset("Repository", repo.key, &["note"]).await?;
            } else {
                tx.set("Repository", repo.key, &[("note", note)]).await?;
            }
            Ok(repo.key)
        }
        None => {
            let root = domain.to_string();
            let mut fields = vec![("name", request.name.as_str()), ("domain", root.as_str())];
            if let Some(note) = request.note.as_deref() {
                fields.push(("note", note));
            }
            tx.put("Repository", &fields).await
        }
    }
}

fn malformed(key: i64) -> Error {
    Error::typed(
        "concord.repository.row",
        format!("Repository Resource {key} is malformed"),
    )
}
