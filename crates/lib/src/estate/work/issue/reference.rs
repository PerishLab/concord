use super::super::Estate;
use super::issue_stale;
use crate::{Error, Result, component};
use crate::{Reference, ReferenceKind};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IssueDeclaration {
    pub issue: super::super::super::Coordinate,
    pub member: String,
    pub provider: String,
    pub owner: String,
    pub repository: String,
    pub number: i64,
    pub revision: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IssueWithdrawal {
    pub issue: super::super::super::Coordinate,
    pub member: String,
    pub owner: String,
    pub repository: String,
    pub number: i64,
    pub revision: i64,
}

impl Estate {
    pub async fn refer_issue(&self, request: &IssueDeclaration) -> Result<i64> {
        validate(
            &request.provider,
            &request.owner,
            &request.repository,
            request.number,
        )?;
        self.executable()?;
        let _guard = self.guard()?;
        self.ensure().await?;
        let anchor = self.issue(&request.issue).await?;
        issue_stale(anchor.revision, request.revision)?;
        let member = self.issue_member(&request.issue, &request.member).await?;
        let rows = self
            .core
            .live("IssueChange")
            .await
            .map_err(super::super::fault)?;
        if rows.iter().any(|row| declared(row, member.key, request)) {
            return Err(Error::typed(
                "concord.reference.reserved",
                "pull or change coordinate is already declared for this Member",
            ));
        }
        let parent = member.key.to_string();
        let root = anchor.key.to_string();
        let number = request.number.to_string();
        let revision = anchor.revision + 1;
        let next = revision.to_string();
        self.core
            .batch(async |tx| {
                tx.put(
                    "IssueChange",
                    &[
                        ("provider", request.provider.as_str()),
                        ("owner", request.owner.as_str()),
                        ("repository", request.repository.as_str()),
                        ("number", number.as_str()),
                        ("member", parent.as_str()),
                        ("anchor", root.as_str()),
                    ],
                )
                .await?;
                tx.set("Anchor", anchor.key, &[("revision", next.as_str())])
                    .await?;
                Ok(())
            })
            .await
            .map_err(super::super::fault)?;
        Ok(revision)
    }

    pub async fn unrefer_issue(&self, request: &IssueWithdrawal) -> Result<i64> {
        self.executable()?;
        let _guard = self.guard()?;
        self.ensure().await?;
        let anchor = self.issue(&request.issue).await?;
        issue_stale(anchor.revision, request.revision)?;
        let member = self.issue_member(&request.issue, &request.member).await?;
        let row = self
            .core
            .live("IssueChange")
            .await
            .map_err(super::super::fault)?
            .into_iter()
            .find(|row| withdrawn(row, member.key, request))
            .ok_or_else(|| {
                Error::typed(
                    "concord.reference.absent",
                    "pull or change coordinate is not declared for this Member",
                )
            })?;
        let revision = anchor.revision + 1;
        let next = revision.to_string();
        self.core
            .batch(async |tx| {
                tx.end("IssueChange", row.key()).await?;
                tx.set("Anchor", anchor.key, &[("revision", next.as_str())])
                    .await?;
                Ok(())
            })
            .await
            .map_err(super::super::fault)?;
        Ok(revision)
    }

    pub(super) async fn issue_references(
        &self,
        member: i64,
        anchor: i64,
    ) -> Result<Vec<Reference>> {
        let mut references = self
            .core
            .live("IssueChange")
            .await
            .map_err(super::super::fault)?
            .into_iter()
            .filter(|row| row.int("member") == Some(member))
            .map(|row| {
                if row.int("anchor") != Some(anchor) {
                    return Err(Error::typed(
                        "concord.reference.row",
                        format!("IssueChange {} has a mismatched Issue anchor", row.key()),
                    ));
                }
                let provider = field(&row, "provider")?;
                let owner = field(&row, "owner")?;
                let repository = field(&row, "repository")?;
                let number = row.int("number").ok_or_else(|| {
                    Error::typed("concord.reference.row", "IssueChange has no number")
                })?;
                validate(&provider, &owner, &repository, number)?;
                Ok(Reference {
                    kind: ReferenceKind::Change,
                    provider,
                    owner,
                    repository,
                    number,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        references.sort_by_key(|reference| {
            (
                reference.owner.clone(),
                reference.repository.clone(),
                reference.number,
            )
        });
        Ok(references)
    }
}

fn declared(row: &keel::Row, member: i64, request: &IssueDeclaration) -> bool {
    [
        row.int("member") == Some(member),
        row.text("provider") == Some(request.provider.as_str()),
        row.text("owner") == Some(request.owner.as_str()),
        row.text("repository") == Some(request.repository.as_str()),
        row.int("number") == Some(request.number),
    ]
    .into_iter()
    .all(|same| same)
}

fn withdrawn(row: &keel::Row, member: i64, request: &IssueWithdrawal) -> bool {
    [
        row.int("member") == Some(member),
        row.text("owner") == Some(request.owner.as_str()),
        row.text("repository") == Some(request.repository.as_str()),
        row.int("number") == Some(request.number),
    ]
    .into_iter()
    .all(|same| same)
}

fn field(row: &keel::Row, name: &str) -> Result<String> {
    row.text(name).map(str::to_string).ok_or_else(|| {
        Error::typed(
            "concord.reference.row",
            format!("IssueChange {} has malformed field {name}", row.key()),
        )
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
