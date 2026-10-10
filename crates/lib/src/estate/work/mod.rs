mod artifact;
pub(in crate::estate) mod integration;
mod issue;
mod overlap;
mod provision;
mod status;

use super::{Estate, fault};
pub use integration::{Guard, Integration, Register, Rename, Repository};
pub use issue::authority;
pub use issue::delivery as issue_delivery;
pub use issue::landing as issue_landing;
pub use issue::{
    Finish, IssueArtifact, IssueClaiming, IssueDeclaration, IssueImport, IssueMemberChange,
    IssueMemberStatus, IssueNarrowing, IssueProving, IssueRelease, IssueRemoval, IssueRetirement,
    IssueWithdrawal, IssueWorktree, Recovery, Start,
};
pub use overlap::{ClaimOverlap, CommittedEvidence, CommittedOverlap};
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
        self.stale(head, claim).is_none()
    }

    pub fn stale(&self, head: &str, claim: &str) -> Option<String> {
        let reason = if self.schema != plumb::boundary::SCHEMA || self.plumb != crate::PLUMB {
            format!(
                "it was made with Plumb {} (Boundary schema {}), but this Concord links Plumb {} (Boundary schema {}); prove it again with this binary",
                self.plumb,
                self.schema,
                crate::PLUMB,
                plumb::boundary::SCHEMA,
            )
        } else if self.head != head {
            format!(
                "Member HEAD moved from {} to {head}; prove it again",
                self.head
            )
        } else if self.claim != claim {
            "the Member Claim changed since it was made; prove it again".to_owned()
        } else {
            return None;
        };
        Some(format!("Member Boundary proof is stale: {reason}"))
    }
}
