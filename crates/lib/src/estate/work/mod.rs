mod artifact;
mod issue;
mod overlap;
mod status;

use super::{Estate, fault};
pub use issue::delivery as issue_delivery;
pub use issue::landing as issue_landing;
pub use issue::{
    IssueArtifact, IssueAttach, IssueClaiming, IssueDeclaration, IssueImport, IssueMemberChange,
    IssueMemberStatus, IssueNarrowing, IssueProving, IssueRelease, IssueRemoval, IssueRetirement,
    IssueWithdrawal, IssueWorktree,
};
pub use overlap::ClaimOverlap;
use serde::Serialize;
pub use status::{BoundaryState, CheckoutState, IntegrationState, UpstreamState};

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Proof {
    pub key: i64,
    pub schema: String,
    pub plumb: String,
    pub base: String,
    pub head: String,
    pub claim: String,
}
