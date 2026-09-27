use keel::atom::{int, string};
use keel::{Graph, resource};

mod current;
mod dependency;
mod domain;
mod member;
pub mod migration;
mod phase;
mod reference;
mod repository;

#[resource]
pub(super) struct Space {
    #[field(string, unique, values = ("space",))]
    name: string,
    #[field(int, min = 0)]
    revision: int,
}

#[resource]
pub(super) struct Domain {
    #[field(string, unique = space)]
    name: string,
    #[field(int, min = 0)]
    revision: int,
    #[relation(Space, many2one, root)]
    space: Space,
}

#[resource]
pub(super) struct Task {
    #[field(string, unique = domain)]
    name: string,
    #[field(string, values = ("active", "retired"))]
    state: string,
    #[field(int, min = 0)]
    revision: int,
    #[relation(Domain, many2one, root)]
    domain: Domain,
    #[relation(Task, many2many, closure, weight = string, origin = string)]
    depends: Task,
}

#[resource]
pub(super) struct Reservation {
    #[field(string, unique = domain)]
    name: string,
    #[relation(Domain, many2one, root)]
    domain: Domain,
}

pub(super) fn graph() -> Graph {
    assemble(true, true, true)
}

pub(super) fn released() -> Graph {
    assemble(true, true, false)
}

pub(super) fn bridge() -> Graph {
    assemble(true, false, false)
}

pub(super) fn legacy() -> Graph {
    assemble(false, false, false)
}

fn assemble(tombstone: bool, references: bool, anchors: bool) -> Graph {
    let mut graph = Graph::new();
    graph
        .plug::<Space>()
        .plug::<Domain>()
        .plug::<Task>()
        .plug::<Reservation>()
        .plug::<domain::Addition>()
        .plug::<repository::Repository>();
    if anchors {
        graph.plug::<reference::Anchor>();
    }
    if tombstone {
        graph.plug::<repository::Tombstone>();
    }
    graph
        .plug::<repository::Addition>()
        .plug::<member::Member>()
        .plug::<member::Addition>()
        .plug::<member::Claim>()
        .plug::<member::Boundary>();
    if references {
        graph.plug::<reference::Issue>().plug::<reference::Change>();
    }
    graph
        .plug::<current::Goal>()
        .plug::<current::Constraint>()
        .plug::<current::Decision>()
        .plug::<current::Focus>()
        .plug::<current::Question>()
        .plug::<current::Next>()
        .plug::<current::Addition>()
        .plug::<phase::Phase>()
        .plug::<phase::Outcome>()
        .plug::<phase::Decision>()
        .plug::<phase::Evidence>()
        .plug::<phase::Carry>()
        .plug::<phase::Addition>()
        .plug::<dependency::Retirement>();
    graph
}

#[cfg(test)]
mod tests {
    use super::{bridge, legacy, migration, released};
    use keel::adapt::db::Sqlite;

    #[tokio::test]
    async fn migration() {
        let temp = tempfile::tempdir().expect("temporary estate");
        let database = temp.path().join("estate.sqlite3");
        let wire = Sqlite::file(&database).await.expect("legacy database");
        let mut legacy = keel::bootstrap(legacy(), wire).expect("legacy graph");
        let sudo = legacy.mint().await.expect("legacy sudo");
        let core = legacy.seal(&sudo).await.expect("legacy estate");
        let space = core
            .put("Space", &[("name", "space"), ("revision", "0")])
            .await
            .expect("legacy Space");
        let domain = core
            .put(
                "Domain",
                &[
                    ("name", "local"),
                    ("revision", "1"),
                    ("space", &space.to_string()),
                ],
            )
            .await
            .expect("legacy Domain");
        let repository = core
            .put(
                "Repository",
                &[
                    ("name", "legacy"),
                    ("note", "RETIRED"),
                    ("domain", &domain.to_string()),
                ],
            )
            .await
            .expect("legacy Repository");
        drop(core);

        let core = migration::bind(&database, bridge)
            .await
            .expect("explicit tombstone migration");
        assert!(core.live("Tombstone").await.expect("Tombstones").is_empty());
        drop(core);

        let core = migration::bind(&database, released)
            .await
            .expect("explicit migration");
        assert!(core.live("Issue").await.expect("Issues").is_empty());
        assert!(core.live("Change").await.expect("Changes").is_empty());
        let row = core
            .live("Repository")
            .await
            .expect("Repositories")
            .remove(0);
        assert_eq!(row.key(), repository);
        assert_eq!(row.text("name"), Some("legacy"));
        assert_eq!(row.text("note"), Some("RETIRED"));
        assert_eq!(row.int("domain"), Some(domain));
    }
}
