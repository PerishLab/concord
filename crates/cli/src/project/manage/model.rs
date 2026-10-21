pub(super) use super::super::field::{Choice, Data};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(in crate::project) struct Intent {
    pub reason: String,
    pub action: Action,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub(in crate::project) enum Action {
    Planning {
        change: super::super::planning::Change,
    },
    Create {
        title: String,
    },
    Configure {
        changes: Changes,
    },
    Link {
        repository: String,
        present: bool,
    },
    Candidates {
        issues: Vec<String>,
        present: bool,
    },
    Field {
        name: String,
        datatype: Data,
        options: Vec<Choice>,
    },
    Rename {
        field: String,
        name: String,
        options: Option<Vec<Choice>>,
    },
    Remove {
        field: String,
    },
    Detach {
        field: String,
        origin: String,
    },
    View {
        name: String,
        layout: Layout,
        filter: String,
        visible: Vec<String>,
        group: Vec<String>,
        vertical: Vec<String>,
        sort: Vec<Sort>,
    },
    Revise {
        view: String,
        name: Option<String>,
        layout: Option<Layout>,
        filter: Option<String>,
        visible: Option<Vec<String>>,
    },
    Retire {
        view: String,
    },
    Workflow {
        workflow: String,
    },
    Record {
        body: String,
    },
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(in crate::project) struct Changes {
    pub title: Option<String>,
    pub description: Option<String>,
    pub readme: Option<String>,
    pub public: Option<bool>,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub(in crate::project) enum Layout {
    Table,
    Board,
    Roadmap,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(in crate::project) struct Sort {
    pub field: String,
    pub direction: Direction,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub(in crate::project) enum Direction {
    Asc,
    Desc,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(in crate::project) struct Plan {
    pub schema: String,
    pub target: String,
    pub before: Value,
    pub intent: Intent,
}

impl Layout {
    pub(super) fn native(&self) -> &'static str {
        match self {
            Self::Table => "TABLE_LAYOUT",
            Self::Board => "BOARD_LAYOUT",
            Self::Roadmap => "ROADMAP_LAYOUT",
        }
    }
}
