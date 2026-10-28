use super::fault;
use crate::runtime::transport::Request;
use concord_core::Result;
use serde_json::{Value, json};
use std::path::Path;

pub(super) struct Client<'a> {
    pub command: &'a Path,
    pub timeout: u64,
}

impl Client<'_> {
    pub async fn query(&self, query: &str, variables: Value) -> Result<Value> {
        let input = serde_json::to_vec(&json!({"query":query,"variables":variables}))
            .expect("native Phase request");
        let output = Request::new(self.command, self.timeout, 1024 * 1024)
            .args(["api", "graphql", "--input", "-"])
            .input(&input)
            .run()
            .await
            .map_err(|error| fault("unavailable", error.to_string()))?;
        let value: Value = serde_json::from_slice(&output.stdout)
            .map_err(|_| fault("unavailable", "invalid native Phase response"))?;
        if value
            .get("errors")
            .is_some_and(|errors| errors.as_array().is_none_or(|errors| !errors.is_empty()))
        {
            return Err(fault(
                "unavailable",
                "native Phase query failed; partial data is not absence",
            ));
        }
        value
            .get("data")
            .filter(|data| data.is_object())
            .cloned()
            .ok_or_else(|| fault("unavailable", "native Phase response has no data"))
    }
}
