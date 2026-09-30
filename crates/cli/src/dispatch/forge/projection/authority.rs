use super::super::github::{envelope, provider};
use super::{Fault, Projection, Pull};
use concord_core::Coordinate;
use plumb::seat::release::Authority;
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};

pub(in crate::dispatch::forge) type Repository = (String, String);

#[derive(Deserialize)]
struct Holder {
    object: Option<Blob>,
}

#[derive(Deserialize)]
struct Blob {
    text: Option<String>,
}

impl Projection<'_> {
    pub(in crate::dispatch::forge) async fn authorities(
        &self,
        trusted: &BTreeSet<Repository>,
    ) -> std::result::Result<Vec<Authority>, Fault> {
        let size = usize::from(self.page_size);
        if trusted.len() > size * usize::from(self.max_pages) {
            return Err(provider(
                "page-limit",
                format!(
                    "GitHub release declarations exceed the {} page limit",
                    self.max_pages
                ),
            ));
        }
        let listed = trusted.iter().collect::<Vec<_>>();
        let mut result = Vec::new();
        for chunk in listed.chunks(size) {
            let texts = self.manifests(chunk).await?;
            for (repository, text) in chunk.iter().zip(texts) {
                result.extend(declare(repository, text.as_deref())?);
            }
        }
        Ok(result)
    }

    async fn manifests(
        &self,
        chunk: &[&Repository],
    ) -> std::result::Result<Vec<Option<String>>, Fault> {
        let mut process = self
            .request()
            .args(["api", "graphql", "-f"])
            .arg(format!("query={}", query(chunk.len())));
        for (index, (owner, name)) in chunk.iter().enumerate() {
            process = process
                .args(["-f", &format!("o{index}={owner}")])
                .args(["-f", &format!("n{index}={name}")]);
        }
        let body = self.run(process).await?;
        let mut data = envelope::<BTreeMap<String, Option<Holder>>>(&body)?.unwrap_or_default();
        chunk
            .iter()
            .enumerate()
            .map(|(index, (owner, name))| {
                data.remove(&format!("r{index}"))
                    .flatten()
                    .map(|holder| holder.object.and_then(|blob| blob.text))
                    .ok_or_else(|| {
                        provider(
                            "missing",
                            format!("GitHub repository {owner}/{name} is not readable"),
                        )
                    })
            })
            .collect()
    }
}

fn query(count: usize) -> String {
    let variables = (0..count)
        .map(|index| format!("$o{index}: String!, $n{index}: String!"))
        .collect::<Vec<_>>()
        .join(", ");
    let fields = (0..count)
        .map(|index| {
            format!(
                "r{index}: repository(owner: $o{index}, name: $n{index}) {{ object(expression: \"HEAD:plumb.toml\") {{ ... on Blob {{ text }} }} }}"
            )
        })
        .collect::<Vec<_>>()
        .join(" ");
    format!("query({variables}) {{ {fields} }}")
}

fn declare(
    repository: &Repository,
    manifest: Option<&str>,
) -> std::result::Result<Option<Authority>, Fault> {
    let Some(manifest) = manifest else {
        return Ok(None);
    };
    Authority::declared(manifest).map_err(|error| {
        provider(
            "declaration",
            format!(
                "{}/{} declares no valid release authority: {error}",
                repository.0, repository.1
            ),
        )
    })
}

pub(in crate::dispatch::forge) fn trust(
    issue: &Coordinate,
    pulls: &[Pull],
) -> BTreeSet<Repository> {
    pulls
        .iter()
        .map(|pull| (pull.owner.clone(), pull.repository.clone()))
        .chain([(issue.owner.clone(), issue.repository.clone())])
        .collect()
}

pub(in crate::dispatch::forge) fn evidence<'a>(
    texts: impl IntoIterator<Item = &'a str>,
    authorities: &[Authority],
) -> Vec<String> {
    texts
        .into_iter()
        .flat_map(str::split_whitespace)
        .map(trim)
        .filter(|token| {
            authorities
                .iter()
                .any(|authority| authority.record(token).is_some())
        })
        .map(str::to_string)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn trim(token: &str) -> &str {
    token.trim_matches(|character: char| {
        matches!(
            character,
            '(' | ')' | '[' | ']' | '<' | '>' | ',' | '.' | ';' | '"' | '\''
        )
    })
}

#[cfg(test)]
mod tests {
    use super::super::Pull;
    use super::{declare, evidence, trust};
    use concord_core::Coordinate;
    use std::collections::BTreeSet;

    const PLUMB: &str = "[release]\nauthority = \"https://releases.plumb.perish.uk\"\n";
    const RECORD: &str =
        "https://releases.plumb.perish.uk/v1/releases/stable/v0.61.0/distribution.json";
    const FOREIGN: &str = "https://releases.example.com/v1/releases/stable/v1/distribution.json";

    fn repository(owner: &str, name: &str) -> (String, String) {
        (owner.to_string(), name.to_string())
    }

    fn pull(owner: &str, name: &str, number: i64) -> Pull {
        Pull {
            node: format!("PR_{number}"),
            owner: owner.to_string(),
            repository: name.to_string(),
            number,
            url: format!("https://github.com/{owner}/{name}/pull/{number}"),
            title: "Deliver".to_string(),
            state: "merged".to_string(),
            merged_at: None,
            updated: "2026-09-29T00:00:00Z".to_string(),
        }
    }

    #[test]
    fn declarations() {
        let plumb = repository("PerishLab", "plumb");
        let declared = declare(&plumb, Some(PLUMB))
            .expect("valid")
            .expect("authority");
        assert_eq!(declared.base(), "https://releases.plumb.perish.uk");
        assert!(declare(&plumb, None).expect("absent").is_none());
        assert!(
            declare(&plumb, Some("[package]\n"))
                .expect("silent")
                .is_none()
        );
        let invalid = "[release]\nauthority = \"http://releases.plumb.perish.uk/\"\n";
        assert_eq!(
            declare(&plumb, Some(invalid)).expect_err("invalid").code,
            "declaration"
        );
        assert_eq!(
            declare(&plumb, Some("[release")).expect_err("broken").code,
            "declaration"
        );
    }

    #[test]
    fn recognition() {
        let plumb = declare(&repository("PerishLab", "plumb"), Some(PLUMB))
            .expect("valid")
            .into_iter()
            .collect::<Vec<_>>();
        let prose = format!(
            "Released ({RECORD}). Also <{FOREIGN}>, https://releases.plumb.perish.uk/v1/channels/stable.json"
        );
        assert_eq!(evidence([prose.as_str()], &plumb), vec![RECORD.to_string()]);
        assert!(evidence([prose.as_str()], &[]).is_empty());
        assert!(evidence([FOREIGN], &plumb).is_empty());
    }

    #[test]
    fn linkage() {
        let issue = Coordinate {
            owner: "PerishLab".to_string(),
            repository: ".github".to_string(),
            number: 5,
        };
        let pulls = [
            pull("PerishLab", "plumb", 73),
            pull("PerishLab", "plumb", 74),
            pull("PerishLab", "concord", 79),
        ];
        let trusted = trust(&issue, &pulls);
        assert_eq!(
            trusted,
            BTreeSet::from([
                repository("PerishLab", ".github"),
                repository("PerishLab", "concord"),
                repository("PerishLab", "plumb"),
            ])
        );
        let authorities = trusted
            .iter()
            .filter(|held| held.1 == "plumb")
            .filter_map(|held| declare(held, Some(PLUMB)).expect("valid"))
            .collect::<Vec<_>>();
        assert_eq!(evidence([RECORD], &authorities), vec![RECORD.to_string()]);
        assert!(!trust(&issue, &[]).contains(&repository("PerishLab", "plumb")));
    }
}
