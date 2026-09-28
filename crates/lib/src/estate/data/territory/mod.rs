use super::{Agreement, Estate};
use crate::Result;
use crate::path::at;

mod anchor;

pub(super) async fn inspect(estate: &Estate, report: &mut Agreement) -> Result<()> {
    custody(estate, report)?;
    anchors(estate, report).await?;
    anchor::inspect(estate, report).await?;
    Ok(())
}

async fn anchors(estate: &Estate, report: &mut Agreement) -> Result<()> {
    for row in estate.core.live("Anchor").await.map_err(super::fault)? {
        if let Err(error) = super::forge::decode_anchor(&row) {
            report.fault(
                "issue.shape",
                format!("Anchor/{}", row.key()),
                error.to_string(),
            );
        }
    }
    Ok(())
}

fn custody(estate: &Estate, report: &mut Agreement) -> Result<()> {
    for (path, mode) in [
        (estate.space.join(".concord"), 0o700),
        (estate.space.join(".concord/estate.sqlite3"), 0o600),
        (estate.space.join(".concord/sudo"), 0o600),
    ] {
        if let Some(found) = at(&path).held()?
            && found != mode
        {
            report.fault(
                "permission",
                path.display().to_string(),
                format!("mode {found:04o} must be {mode:04o}"),
            );
        }
    }
    Ok(())
}
