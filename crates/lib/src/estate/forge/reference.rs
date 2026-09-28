use serde::Serialize;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ReferenceKind {
    Change,
}

impl ReferenceKind {
    pub fn name(self) -> &'static str {
        match self {
            Self::Change => "change",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Reference {
    pub kind: ReferenceKind,
    pub provider: String,
    pub owner: String,
    pub repository: String,
    pub number: i64,
}
