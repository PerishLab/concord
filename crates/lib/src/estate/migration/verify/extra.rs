use super::mismatch;
use crate::Result;
use keel::Row;
use std::collections::BTreeMap;

pub(super) fn check(
    rows: &[Row],
    relation: &str,
    root: i64,
    expected: &BTreeMap<String, toml::Value>,
) -> Result<()> {
    let mut held = rows
        .iter()
        .filter(|row| row.int(relation) == Some(root))
        .map(|row| {
            (
                row.int("rank").unwrap_or(0),
                row.text("name").unwrap_or("").to_string(),
                row.text("body").unwrap_or("").to_string(),
            )
        })
        .collect::<Vec<_>>();
    held.sort();
    let wanted = expected
        .iter()
        .enumerate()
        .map(|(index, (name, value))| (index as i64 + 1, name.clone(), value.to_string()))
        .collect::<Vec<_>>();
    if held == wanted {
        return Ok(());
    }
    Err(mismatch(format!(
        "Addition facts differ for {relation} Resource {root}"
    )))
}
