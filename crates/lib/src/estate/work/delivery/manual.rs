use super::Context;
use crate::{Error, Result};
use plumb::delivery::native::{Context as Native, Evidence, Gate, Identity};
use plumb::landing::Refusal;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub(super) const AUTHORITY: &str = concat!(
    "concord.governance-manual/v1@",
    env!("CONCORD_MANUAL_GATE"),
    "/",
    env!("CONCORD_PLUMB_VERSION")
);
const REPOSITORY: &str = "R_kgDOUsHIOQ";

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Review {
    pub schema: String,
    pub verification: String,
    pub repository: String,
    pub issue: String,
    pub source: String,
    pub target: String,
    pub tree: String,
    pub reason: String,
}

pub(super) struct Governance<'a>(pub &'a Review);

#[derive(Serialize)]
struct Receipt<'a> {
    authority: &'static str,
    review: &'a Review,
    git: plumb::config::Tool,
}

impl Review {
    pub const SHAPE: &str = r#"{"schema":"concord.manual-review/v1","verification":"manual","repository":"R_kgDOUsHIOQ","issue":"I_node","source":"40-hex-source","target":"40-hex-base","tree":"40-hex-tree","reason":"What was manually reviewed and why this exact change is acceptable"}"#;

    pub(super) fn check(&self, context: &Context) -> Result<()> {
        admit(&context.member.integration)?;
        self.valid()?;
        if self.repository != context.member.integration.node || self.issue != context.node {
            return Err(refused(
                "manual review must name the exact repository and Issue node",
            ));
        }
        Ok(())
    }

    pub(super) fn valid(&self) -> Result<()> {
        if self.schema != "concord.manual-review/v1" || self.verification != "manual" {
            return Err(refused(
                "review requires schema concord.manual-review/v1 and verification manual",
            ));
        }
        if self.repository != REPOSITORY || self.issue.is_empty() {
            return Err(refused(
                "review requires the exact .github repository and Issue nodes",
            ));
        }
        if self.reason.trim().is_empty() || self.reason.len() > 4096 {
            return Err(refused("manual review reason must contain 1..4096 bytes"));
        }
        Ok(())
    }

    pub(super) fn render(&self) -> Result<String> {
        let text =
            serde_json::to_string_pretty(self).map_err(|error| refused(error.to_string()))?;
        Ok(format!(
            "\n\nRepository authority: `{AUTHORITY}`\n\nVerification: manual review. No automated gate checks are claimed. The operator reviewed these exact inputs under the canonical manual-maintenance exception.\n\n```json\n{text}\n```"
        ))
    }
}

pub(super) fn admit(integration: &crate::estate::Integration) -> Result<()> {
    if integration.node == REPOSITORY
        && integration.repository.owner == "PerishLab"
        && integration.repository.name == ".github"
    {
        return Ok(());
    }
    Err(Error::typed(
        "concord.delivery.authority",
        "governance-manual requires the registered PerishLab/.github repository node",
    ))
}

impl Gate for Governance<'_> {
    fn verify(&self, context: &Native<'_>) -> std::result::Result<Evidence, Refusal> {
        self.identify(context)
    }
}

impl Identity for Governance<'_> {
    fn identify(&self, context: &Native<'_>) -> std::result::Result<Evidence, Refusal> {
        self.0.valid().map_err(fault)?;
        if self.0.source != context.source
            || self.0.target != context.target
            || self.0.tree != context.tree
        {
            return Err(fault(refused(
                "manual review source/base/tree changed; review the current exact inputs and prepare again",
            )));
        }
        let git = tool(context.root)?;
        policy(context)?;
        if git != tool(context.root)? {
            return Err(fault(refused(
                "Git identity changed while checking manual review",
            )));
        }
        let receipt = Receipt {
            authority: AUTHORITY,
            review: self.0,
            git,
        };
        let bytes =
            serde_json::to_vec(&receipt).map_err(|error| fault(refused(error.to_string())))?;
        Ok(Evidence {
            authority: AUTHORITY.to_string(),
            source: context.source.to_string(),
            tree: context.tree.to_string(),
            digest: format!("{:x}", Sha256::digest(bytes)),
        })
    }
}

fn tool(root: &std::path::Path) -> std::result::Result<plumb::config::Tool, Refusal> {
    let search = plumb::config::search(root).map_err(|error| fault(refused(error)))?;
    let path = search
        .resolve("git")
        .map_err(|error| fault(refused(error)))?;
    let path = path
        .canonicalize()
        .map_err(|error| fault(refused(error.to_string())))?;
    plumb::config::Tool::at(path).map_err(|error| fault(refused(error)))
}

fn policy(context: &Native<'_>) -> std::result::Result<(), Refusal> {
    for commit in [context.source, context.target] {
        for (file, required) in [
            (
                "AGENTS.md",
                "This repository is maintained manually and is outside product governance.",
            ),
            ("GOVERNANCE.md", "This repository is maintained manually."),
        ] {
            let text = crate::git::at(context.root)
                .text(&["show", &format!("{commit}:{file}")])
                .map_err(fault)?;
            if !text
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ")
                .contains(required)
            {
                return Err(fault(refused(format!(
                    "manual maintenance declaration is absent from {commit}:{file}; inspect the canonical exception"
                ))));
            }
        }
    }
    Ok(())
}

fn refused(message: impl Into<String>) -> Error {
    Error::typed("concord.delivery.review", message)
}

fn fault(error: Error) -> Refusal {
    Refusal {
        kind: "review",
        message: error.to_string(),
    }
}
