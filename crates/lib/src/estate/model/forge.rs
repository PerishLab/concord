use super::{Estate, Life, World, fault};
use crate::{Error, Result, component};
use keel::Row;
use serde::Serialize;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ReferenceKind {
    Issue,
    Change,
}

impl ReferenceKind {
    pub fn name(self) -> &'static str {
        match self {
            Self::Issue => "issue",
            Self::Change => "change",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Reference {
    pub kind: ReferenceKind,
    pub provider: String,
    pub owner: String,
    pub repository: String,
    pub number: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ForgeDeclaration {
    pub task: String,
    pub member: Option<String>,
    pub provider: String,
    pub owner: String,
    pub repository: String,
    pub number: i64,
    pub revision: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ForgeWithdrawal {
    pub task: String,
    pub member: Option<String>,
    pub revision: i64,
}

impl Estate {
    pub async fn refer(&self, declaration: &ForgeDeclaration) -> Result<i64> {
        validate(
            &declaration.provider,
            &declaration.owner,
            &declaration.repository,
            declaration.number,
        )?;
        let _guard = self.guard()?;
        self.ensure().await?;
        let world = World::load(self).await?;
        let task = world.node(&declaration.task)?;
        active(task.life, &declaration.task)?;
        stale(task.revision, declaration.revision)?;
        let (unit, relation, parent) = match declaration.member.as_deref() {
            Some(name) => (
                "Change",
                "member",
                self.member(&task.identity(), name).await?.key,
            ),
            None => ("Issue", "task", task.key),
        };
        let rows = self.core.live(unit).await.map_err(fault)?;
        let held = rows.iter().find(|row| row.int(relation) == Some(parent));
        let revision = task.revision + 1;
        let next = revision.to_string();
        let number = declaration.number.to_string();
        let root = parent.to_string();
        self.core
            .batch(async |tx| {
                let fields = [
                    ("provider", declaration.provider.as_str()),
                    ("owner", declaration.owner.as_str()),
                    ("repository", declaration.repository.as_str()),
                    ("number", number.as_str()),
                ];
                if let Some(row) = held {
                    tx.set(unit, row.key(), &fields).await?;
                } else {
                    let mut fields = fields.to_vec();
                    fields.push((relation, root.as_str()));
                    tx.put(unit, &fields).await?;
                }
                tx.set("Task", task.key, &[("revision", next.as_str())])
                    .await?;
                Ok(())
            })
            .await
            .map_err(fault)?;
        Ok(revision)
    }

    pub async fn unrefer(&self, withdrawal: &ForgeWithdrawal) -> Result<i64> {
        let _guard = self.guard()?;
        self.ensure().await?;
        let world = World::load(self).await?;
        let task = world.node(&withdrawal.task)?;
        active(task.life, &withdrawal.task)?;
        stale(task.revision, withdrawal.revision)?;
        let (unit, relation, parent) = match withdrawal.member.as_deref() {
            Some(name) => (
                "Change",
                "member",
                self.member(&task.identity(), name).await?.key,
            ),
            None => ("Issue", "task", task.key),
        };
        let row = self
            .core
            .live(unit)
            .await
            .map_err(fault)?
            .into_iter()
            .find(|row| row.int(relation) == Some(parent))
            .ok_or_else(|| {
                Error::typed(
                    "concord.reference.absent",
                    format!("{unit} reference is not declared"),
                )
            })?;
        let revision = task.revision + 1;
        let next = revision.to_string();
        self.core
            .batch(async |tx| {
                tx.end(unit, row.key()).await?;
                tx.set("Task", task.key, &[("revision", next.as_str())])
                    .await?;
                Ok(())
            })
            .await
            .map_err(fault)?;
        Ok(revision)
    }

    pub(super) async fn forge(&self, task: i64, member: Option<i64>) -> Result<Option<Reference>> {
        let (unit, relation, parent, kind) = match member {
            Some(member) => ("Change", "member", member, ReferenceKind::Change),
            None => ("Issue", "task", task, ReferenceKind::Issue),
        };
        let mut rows = self
            .core
            .live(unit)
            .await
            .map_err(fault)?
            .into_iter()
            .filter(|row| row.int(relation) == Some(parent));
        let Some(row) = rows.next() else {
            return Ok(None);
        };
        if rows.next().is_some() {
            return Err(Error::typed(
                "concord.reference.duplicate",
                format!("{unit} parent {parent} has multiple references"),
            ));
        }
        decode(&row, kind).map(Some)
    }
}

pub(super) fn decode(row: &Row, kind: ReferenceKind) -> Result<Reference> {
    let text = |field| {
        row.text(field)
            .map(str::to_string)
            .ok_or_else(|| malformed(row.key(), field))
    };
    let provider = text("provider")?;
    let owner = text("owner")?;
    let repository = text("repository")?;
    let number = row
        .int("number")
        .ok_or_else(|| malformed(row.key(), "number"))?;
    validate(&provider, &owner, &repository, number)?;
    Ok(Reference {
        kind,
        provider,
        owner,
        repository,
        number,
    })
}

fn validate(provider: &str, owner: &str, repository: &str, number: i64) -> Result<()> {
    component("forge provider", provider)?;
    component("forge owner", owner)?;
    component("forge repository", repository)?;
    if provider != "github" {
        return Err(Error::typed(
            "concord.reference.provider",
            format!("unsupported forge provider {provider}"),
        ));
    }
    if owner.chars().any(char::is_whitespace) || repository.chars().any(char::is_whitespace) {
        return Err(Error::typed(
            "concord.reference.coordinate",
            "forge owner and repository cannot contain whitespace",
        ));
    }
    if number < 1 {
        return Err(Error::typed(
            "concord.reference.number",
            "forge reference number must be positive",
        ));
    }
    Ok(())
}

fn active(life: Life, task: &str) -> Result<()> {
    if life == Life::Active {
        return Ok(());
    }
    Err(Error::typed(
        "concord.task.retired",
        format!("retired Task is immutable: {task}"),
    ))
}

fn stale(found: i64, expected: i64) -> Result<()> {
    if found == expected {
        return Ok(());
    }
    Err(Error::typed(
        "concord.task.stale",
        format!("Task revision changed: expected {expected}, found {found}"),
    ))
}

fn malformed(key: i64, field: &str) -> Error {
    Error::typed(
        "concord.reference.row",
        format!("forge Reference {key} has malformed field {field}"),
    )
}

#[cfg(test)]
mod tests {
    use super::super::Seat;

    #[tokio::test]
    async fn invalid() {
        let temp = tempfile::tempdir().expect("temporary space");
        let estate = Seat::new(temp.path())
            .bootstrap()
            .await
            .expect("bootstrap estate");
        estate.manage("local").await.expect("manage Domain");
        let task = estate.start("local", "alpha").await.expect("start Task");
        estate
            .core
            .put(
                "Issue",
                &[
                    ("provider", "bad/provider"),
                    ("owner", "PerishLab"),
                    ("repository", "concord"),
                    ("number", "9"),
                    ("task", &task.key.to_string()),
                ],
            )
            .await
            .expect("inject malformed reference");
        let report = estate.inspect(None, None).await.expect("audit estate");
        assert!(
            report
                .faults
                .iter()
                .any(|finding| finding.code == "reference.shape")
        );
    }
}
