use super::super::emit;
use concord_core::{Coordinate, Estate, Result};
use serde_json::json;

pub(super) async fn run(estate: &Estate, issue: &str, member: &str, output: bool) -> Result<()> {
    let status = estate
        .issue_member_status(&Coordinate::parse(issue)?, member)
        .await?;
    if output {
        return emit(json!({"status": status}), true);
    }
    super::super::output::issue_member_status(&status);
    Ok(())
}
