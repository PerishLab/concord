use crate::{Error, Result};
use serde::Serialize;

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Held {
    pub holder: &'static str,
    pub operation: Option<String>,
    pub state: String,
}

pub fn held(kind: &str, body: &str, state: &str) -> Option<Held> {
    (kind == "Auto").then(|| Held {
        holder: "automation-held",
        operation: crate::acceptance::sections(body)
            .remove("operation")
            .filter(|operation| !operation.trim().is_empty()),
        state: state.to_ascii_lowercase(),
    })
}

pub fn branch(value: &str) -> Option<u64> {
    let text = value.strip_prefix("auto/")?;
    text.parse::<u64>()
        .ok()
        .filter(|number| *number > 0 && number.to_string() == text)
}

pub fn admit(kind: &str) -> Result<()> {
    if kind == "Auto" {
        return Err(Error::typed(
            "concord.issue.automation",
            "Auto is automation-held: Plumb owns follow and its lifecycle; Concord only projects it read-only and creates no Anchor or Member",
        ));
    }
    Ok(())
}

pub fn paths(root: &std::path::Path) -> Result<Vec<String>> {
    Ok(crate::git::at(root)
        .text(&["ls-tree", "-r", "--name-only", "-z", "HEAD"])?
        .split('\0')
        .filter(|path| registered(path))
        .map(str::to_string)
        .collect())
}

pub fn overlap(claims: &[String], paths: &[String]) -> Vec<String> {
    paths
        .iter()
        .filter(|path| claims.iter().any(|claim| crate::claim::covers(claim, path)))
        .cloned()
        .collect()
}

fn registered(path: &str) -> bool {
    matches!(
        path.rsplit('/').next(),
        Some("Cargo.toml" | "Cargo.lock" | "package.json" | "pnpm-lock.yaml")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registered() {
        assert_eq!(branch("auto/17"), Some(17));
        for value in [
            "auto/0",
            "auto/017",
            "auto/+17",
            "auto/word",
            "land/auto/17",
            "task/17",
        ] {
            assert_eq!(branch(value), None);
        }
        let held = held("Auto", "## Operation\nfollow\n", "OPEN").unwrap();
        assert_eq!(held.operation.as_deref(), Some("follow"));
        assert_eq!(held.state, "open");
        assert_eq!(held.holder, "automation-held");
        assert!(admit("Auto").is_err());
        assert!(admit("Task").is_ok());
        assert!(super::held("Task", "## Operation\nfollow\n", "OPEN").is_none());
    }
}
