use super::Estate;
use crate::{Error, Result};
use serde::Serialize;

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Finding {
    pub code: String,
    pub subject: String,
    pub message: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Agreement {
    pub target: String,
    pub revision: i64,
    pub faults: Vec<Finding>,
    pub observations: Vec<Finding>,
}

impl Agreement {
    pub fn agrees(&self) -> bool {
        self.faults.is_empty()
    }

    pub(super) fn fault(
        &mut self,
        code: &str,
        subject: impl Into<String>,
        message: impl Into<String>,
    ) {
        self.faults.push(Finding {
            code: code.to_string(),
            subject: subject.into(),
            message: message.into(),
        });
    }

    pub(super) fn observe(
        &mut self,
        code: &str,
        subject: impl Into<String>,
        message: impl Into<String>,
    ) {
        self.observations.push(Finding {
            code: code.to_string(),
            subject: subject.into(),
            message: message.into(),
        });
    }
}

impl Estate {
    #[locus::trace(with = crate::observation::view())]
    pub async fn inspect(&self, task: Option<&str>, domain: Option<&str>) -> Result<Agreement> {
        if task.is_some() || domain.is_some() {
            return Err(Error::typed(
                "concord.audit.scope",
                "the Issue estate has no Task or Domain audit scope",
            ));
        }
        let mut report = Agreement {
            target: self.space.display().to_string(),
            revision: 0,
            faults: Vec::new(),
            observations: Vec::new(),
        };
        super::territory::inspect(self, &mut report).await?;
        report.faults.sort_by_key(order);
        report.observations.sort_by_key(order);
        Ok(report)
    }

    pub(super) async fn ensure(&self) -> Result<()> {
        let report = self.inspect(None, None).await?;
        if report.agrees() {
            return Ok(());
        }
        Err(Error::typed(
            "concord.audit.refused",
            format!("estate has {} agreement fault(s)", report.faults.len()),
        ))
    }
}

fn order(finding: &Finding) -> (String, String, String) {
    (
        finding.code.clone(),
        finding.subject.clone(),
        finding.message.clone(),
    )
}
