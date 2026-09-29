use super::pull::stale;
use concord_core::Result;
use plumb::landing::Preparation;
use serde::Deserialize;
use serde::Serialize;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum State {
    Open,
    Closed,
    Merged,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Commit {
    pub oid: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Pull {
    pub id: String,
    pub number: i64,
    pub url: String,
    pub state: State,
    #[serde(rename = "baseRefName")]
    pub base: String,
    #[serde(rename = "headRefOid")]
    pub head: String,
    #[serde(rename = "mergeCommit")]
    pub merge: Option<Commit>,
    #[serde(rename = "mergedAt")]
    pub merged: Option<String>,
    #[serde(rename = "updatedAt")]
    pub updated: String,
    pub title: String,
    pub body: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Report {
    pub pull: Pull,
    pub candidate: String,
}

pub(super) fn contract(pull: &Pull, preparation: &Preparation, repository: &str) -> Result<()> {
    let expected = format!("https://github.com/{repository}/pull/{}", pull.number);
    if pull.id.is_empty() || pull.number < 1 {
        return refusal();
    }
    if pull.url != expected || pull.base != preparation.base {
        return refusal();
    }
    if pull.head != preparation.candidate || pull.title != preparation.title {
        return refusal();
    }
    if pull.body != preparation.body || pull.updated.is_empty() {
        return refusal();
    }
    let merged = pull.state == State::Merged;
    if merged != pull.merge.is_some() || merged != pull.merged.is_some() {
        return refusal();
    }
    Ok(())
}

fn refusal() -> Result<()> {
    Err(stale(
        "pull identity, base, head, narrative, state evidence, or update identity does not match the exact delivery plan",
    ))
}

#[cfg(test)]
mod tests {
    use super::{Commit, Pull, State, contract};
    use plumb::landing::{Guard, Preparation};
    use std::path::PathBuf;

    #[test]
    fn decoding() {
        let pull: Pull = serde_json::from_str(
            r#"{"id":"PR_node","number":28,"url":"https://github.com/PerishLab/concord/pull/28","state":"MERGED","baseRefName":"main","headRefOid":"abc","mergeCommit":{"oid":"def"},"mergedAt":"2026-09-29T00:00:00Z","updatedAt":"2026-09-29T00:00:01Z","title":"Deliver","body":"Refs PerishLab/concord#28"}"#,
        )
        .expect("provider Pull");
        assert_eq!(pull.id, "PR_node");
        assert_eq!(pull.head, "abc");
        assert_eq!(pull.state, State::Merged);
    }

    #[test]
    fn exact() {
        let preparation = preparation();
        let pull = pull();
        contract(&pull, &preparation, "PerishLab/concord").expect("exact pull");
        for drifted in [
            Pull {
                head: "other".into(),
                ..pull.clone()
            },
            Pull {
                base: "other".into(),
                ..pull.clone()
            },
            Pull {
                body: "other".into(),
                ..pull.clone()
            },
            Pull {
                url: "https://github.com/Other/concord/pull/28".into(),
                ..pull.clone()
            },
            Pull {
                merge: None,
                ..pull
            },
        ] {
            assert_eq!(
                contract(&drifted, &preparation, "PerishLab/concord")
                    .expect_err("drifted pull")
                    .code(),
                "concord.delivery.stale"
            );
        }
    }

    fn pull() -> Pull {
        Pull {
            id: "PR_node".into(),
            number: 28,
            url: "https://github.com/PerishLab/concord/pull/28".into(),
            state: State::Merged,
            base: "main".into(),
            head: "candidate".into(),
            merge: Some(Commit {
                oid: "merge".into(),
            }),
            merged: Some("2026-09-29T00:00:00Z".into()),
            updated: "2026-09-29T00:00:01Z".into(),
            title: "Deliver".into(),
            body: "Refs PerishLab/concord#28".into(),
        }
    }

    fn preparation() -> Preparation {
        Preparation {
            root: PathBuf::from("/tmp/member"),
            base: "main".into(),
            target: "target".into(),
            branch: "topic".into(),
            projection: "land/topic".into(),
            source: "source".into(),
            candidate: "candidate".into(),
            title: "Deliver".into(),
            body: "Refs PerishLab/concord#28".into(),
            guard: Guard {
                schema: "guard".into(),
                tree: "tree".into(),
                digest: "digest".into(),
            },
        }
    }
}
