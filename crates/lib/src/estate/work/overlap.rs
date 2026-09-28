use serde::Serialize;

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ClaimOverlap {
    pub code: String,
    pub peer: String,
    pub paths: Vec<String>,
}
