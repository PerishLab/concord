use serde::{Deserialize, Serialize};

#[derive(Clone, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub(in crate::project) enum Change {
    Priority {
        field: String,
        item: String,
        level: Option<Level>,
    },
    Cycle {
        field: Option<String>,
        name: String,
        start: String,
        duration: u16,
        periods: Vec<Period>,
    },
    Assign {
        field: String,
        item: String,
        period: Option<String>,
    },
    Order {
        field: String,
        period: String,
        item: String,
        after: Option<String>,
    },
}

#[derive(Clone, Deserialize, Serialize)]
pub(in crate::project) enum Level {
    Urgent,
    High,
    Medium,
    Low,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(in crate::project) struct Period {
    pub id: Option<String>,
    pub title: String,
    pub start: String,
    pub duration: u16,
}
