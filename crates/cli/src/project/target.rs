use super::fault;
use concord_core::Result;
use serde_json::Value;

pub(super) struct Target {
    pub owner: String,
    pub number: u32,
}

impl Target {
    pub fn parse(raw: &str) -> Result<Self> {
        let coordinate = if let Some(path) = raw.strip_prefix("https://github.com/orgs/") {
            path.replace("/projects/", "/")
        } else {
            raw.to_string()
        };
        let Some((owner, number)) = coordinate.split_once('/') else {
            return Err(fault(
                "target",
                "select OWNER/NUMBER or a canonical organization Project URL",
            ));
        };
        let valid = owner
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-');
        if owner.is_empty() || owner.len() > 39 || !valid {
            return Err(fault("target", "invalid organization login"));
        }
        if owner.starts_with('-') || owner.ends_with('-') {
            return Err(fault("target", "invalid organization login boundary"));
        }
        let number = number.parse::<u32>().ok().filter(|number| *number > 0);
        if number.is_none() {
            return Err(fault("target", "invalid organization Project coordinate"));
        }
        Ok(Self {
            owner: owner.into(),
            number: number.expect("checked number"),
        })
    }

    pub fn verify(&self, project: &Value) -> Result<()> {
        let owner = project["owner"]["login"].as_str().unwrap_or("");
        let url = format!("https://github.com/orgs/{owner}/projects/{}", self.number);
        if project["__typename"] != "ProjectV2" || project["owner"]["__typename"] != "Organization"
        {
            return Err(fault(
                "identity",
                "provider returned an unsupported Project or owner type",
            ));
        }
        if !owner.eq_ignore_ascii_case(&self.owner)
            || project["number"].as_u64() != Some(self.number.into())
            || project["url"] != url
        {
            return Err(fault(
                "identity",
                "provider Project coordinate does not agree with the explicit target",
            ));
        }
        if project["id"].as_str().is_none_or(str::is_empty)
            || project["updatedAt"].as_str().is_none_or(str::is_empty)
        {
            return Err(fault(
                "identity",
                "provider Project stable identity or timestamp is unreadable",
            ));
        }
        Ok(())
    }
}
