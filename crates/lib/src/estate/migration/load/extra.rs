use keel::Tx;
use keel::adapt::db::Sqlite;
use std::collections::BTreeMap;

pub(super) struct Parent<'a> {
    unit: &'a str,
    relation: &'a str,
    key: i64,
}

impl<'a> Parent<'a> {
    pub fn new(unit: &'a str, relation: &'a str, key: i64) -> Self {
        Self {
            unit,
            relation,
            key,
        }
    }
}

pub(super) async fn write(
    tx: &mut Tx<'_, Sqlite>,
    parent: Parent<'_>,
    extra: &BTreeMap<String, toml::Value>,
) -> std::result::Result<(), keel::adapt::Error> {
    for (index, (name, value)) in extra.iter().enumerate() {
        let rank = (index + 1).to_string();
        let body = value.to_string();
        let root = parent.key.to_string();
        tx.put(
            parent.unit,
            &[
                ("rank", rank.as_str()),
                ("name", name.as_str()),
                ("body", body.as_str()),
                (parent.relation, root.as_str()),
            ],
        )
        .await?;
    }
    Ok(())
}
