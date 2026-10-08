use super::{Integration, guard};
use crate::estate::{Estate, fault};
use crate::{Error, Result};

impl Estate {
    pub async fn retire(&self, expected: &Integration, apply: bool) -> Result<Integration> {
        let _integration = guard(self, expected)?;
        let _space = self.guard()?;
        let held = self.locate(&expected.node).await?;
        if &held != expected {
            return Err(Error::typed(
                "concord.integration.changed",
                "Integration identity changed; reread its key, node and coordinate before retirement",
            ));
        }
        let members = self.core.live("IssueMember").await.map_err(fault)?;
        if members
            .iter()
            .any(|row| row.int("integration") == Some(held.key))
        {
            return Err(Error::typed(
                "concord.integration.dependent",
                "Integration has live Members; settle their execution resources before retirement",
            ));
        }
        let mut report = self.inspect().await?;
        report.faults.retain(|finding| {
            finding.code != "integration.agreement" || finding.subject != held.repository.identity()
        });
        if !report.agrees() {
            return Err(Error::detailed(
                "concord.audit.refused",
                "Integration retirement refuses unrelated agreement faults",
                serde_json::json!({"agreement": report}),
            ));
        }
        if apply {
            self.core
                .batch(async |tx| {
                    tx.end("Integration", held.key).await?;
                    Ok(())
                })
                .await
                .map_err(fault)?;
        }
        Ok(held)
    }
}
