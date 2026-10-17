use serde_json::json;

use super::{Record, Target};

const CODE: &str = "concord.acceptance.target";

pub fn refuse(label: Option<Target>, declared: Option<&Record>) -> crate::Error {
    let message = match (label, declared) {
        (Some(label), None) => format!(
            "managed label {0} is present but the Issue has no acceptance declaration; declare target {1} to adopt the label, or remove {0} before declaring another target",
            label.label(),
            label.name(),
        ),
        (None, Some(record)) => format!(
            "current declaration {0} targets {1} but no managed label is present; re-run acceptance apply with the plan that published {0}, which restores {2}",
            record.reference.node,
            record.marker.target().name(),
            record.marker.target().label(),
        ),
        (label, record) => format!(
            "managed label {0} disagrees with current declaration {1}, which targets {2}; replace {0} with {3} on the Issue, then change the target only through acceptance amend",
            label.map_or("none", Target::label),
            record.map_or("none", |record| record.reference.node.as_str()),
            record.map_or("none", |record| record.marker.target().name()),
            record.map_or("none", |record| record.marker.target().label()),
        ),
    };
    let details = json!({
        "label": label.map(Target::label),
        "declared": declared.map(|record| record.marker.target().label()),
        "declaration": declared.map(|record| &record.reference),
    });
    crate::Error::detailed(CODE, message, details)
}
