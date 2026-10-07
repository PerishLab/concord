mod body;
mod evaluation;
mod history;
mod marker;
mod model;
mod plan;
mod report;
mod validation;

pub use body::{checkboxes, sections};
pub use evaluation::{Check, Evaluation, Fact, Observation, Verdict};
pub use history::{Comment, History, Record};
pub use model::{Judgment, Marker, Reference, Release, Target, Verification};
pub use plan::{Plan, Recovery, Step};
pub use report::Report;

use sha2::{Digest, Sha256};

pub fn digest(text: &str) -> String {
    format!("{:x}", Sha256::digest(text.as_bytes()))
}

fn fault(message: impl Into<String>) -> crate::Error {
    crate::Error::typed("concord.acceptance.protocol", message)
}
