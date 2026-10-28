use super::super::delivery::pull::stale;
use concord_core::{Error, Reference, ReferenceKind, Result};

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
        kind: ReferenceKind::Change,
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
    if reference.kind != ReferenceKind::Change
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
