use serde::{Deserialize, Serialize};

use super::{Evaluation, Reference, Verdict};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Report {
    pub evaluation: Evaluation,
    pub promise: String,
    pub checklist: String,
}

impl Report {
    pub fn render(&self) -> String {
        let target = self
            .evaluation
            .target
            .map(|target| target.label())
            .unwrap_or("not adopted");
        let verification = self
            .evaluation
            .verification
            .map(|value| format!("{value:?}").to_ascii_lowercase())
            .unwrap_or_else(|| "none".into());
        let verdict = format!("{:?}", self.evaluation.verdict).to_ascii_lowercase();
        let declaration = reference(self.evaluation.declaration.as_ref());
        let closure = reference(self.evaluation.closure.as_ref());
        let mut remaining = self
            .evaluation
            .checks
            .iter()
            .filter(|check| {
                check.fact.verdict != Verdict::Satisfied
                    && (self.evaluation.target.is_some()
                        || !matches!(check.name.as_str(), "closure" | "freshness"))
            })
            .map(|check| {
                format!(
                    "- {}: {:?}: {}",
                    check.name, check.fact.verdict, check.fact.reason
                )
            })
            .collect::<Vec<_>>();
        if remaining.is_empty() {
            remaining.push(
                "No unresolved observed checks; this is not a prediction of merge or publication."
                    .into(),
            );
        }
        format!(
            "## Acceptance\n\nTarget: `{target}`.\n\nDeclaration: {declaration}.\n\nClosure: {closure}.\n\nCurrent verdict: `{verdict}`; verification: `{verification}`.\n\nPromise:\n\n{}\n\n### Remaining obligations\n\n{}\n\n### Issue checklist\n\n{}\n\nThis PR uses Refs. Merge neither publishes a release nor creates a closure judgment; final Issue closure remains explicit.",
            self.promise,
            remaining.join("\n"),
            self.checklist
        )
    }
}

fn reference(value: Option<&Reference>) -> String {
    value
        .map(|value| format!("`{}` (body `{}`)", value.node, value.digest))
        .unwrap_or_else(|| "none".into())
}
