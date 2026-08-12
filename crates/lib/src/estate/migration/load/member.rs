use super::extra::{self, Parent};
use crate::Member;
use keel::Tx;
use keel::adapt::db::Sqlite;

pub(super) async fn write(
    tx: &mut Tx<'_, Sqlite>,
    member: &Member,
    task: i64,
) -> std::result::Result<(), keel::adapt::Error> {
    let task = task.to_string();
    let mut fields = vec![
        ("name", member.name.as_str()),
        ("source", member.source.as_str()),
        ("task", task.as_str()),
    ];
    if let Some(branch) = member.branch.as_deref() {
        fields.push(("branch", branch));
    }
    let key = tx.put("Member", &fields).await?;
    let parent = key.to_string();
    for held in &member.write {
        tx.put(
            "Claim",
            &[("path", held.as_str()), ("member", parent.as_str())],
        )
        .await?;
    }
    if let Some(held) = &member.boundary {
        tx.put(
            "Boundary",
            &[
                ("schema", held.schema.as_str()),
                ("plumb", held.plumb.as_str()),
                ("base", held.base.as_str()),
                ("head", held.head.as_str()),
                ("claim", held.claim.as_str()),
                ("member", parent.as_str()),
            ],
        )
        .await?;
    }
    extra::write(
        tx,
        Parent::new("member:addition", "member", key),
        &member.extra,
    )
    .await
}
