mod artifact;
pub(in crate::estate) mod integration;
mod issue;
mod overlap;
mod status;

use super::{Estate, fault};
pub use integration::{Integration, Register, Rename, Repository};
pub use issue::authority;
pub use issue::delivery as issue_delivery;
pub use issue::landing as issue_landing;
pub use issue::{
    IssueArtifact, IssueAttach, IssueClaiming, IssueDeclaration, IssueImport, IssueMemberChange,
    IssueMemberStatus, IssueNarrowing, IssueProving, IssueRelease, IssueRemoval, IssueRetirement,
    IssueWithdrawal, IssueWorktree,
};
pub use overlap::ClaimOverlap;
use serde::{Deserialize, Serialize};
pub use status::{BoundaryState, CheckoutState, IntegrationState, UpstreamState};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Proof {
    pub key: i64,
    pub schema: String,
    pub plumb: String,
    pub base: String,
    pub head: String,
    pub claim: String,
}

impl Proof {
    pub fn current(&self, head: &str, claim: &str) -> bool {
        self.linked() && self.head == head && self.claim == claim
    }

    fn linked(&self) -> bool {
        self.schema == plumb::boundary::SCHEMA && self.plumb == crate::PLUMB
    }
}
