use keel::atom::{int, string};
use keel::{Graph, resource};

mod current;
mod dependency;
mod domain;
mod execution;
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
    assemble(true, true, true, true)
}

pub(super) fn anchored() -> Graph {
    assemble(true, true, true, false)
}

pub(super) fn released() -> Graph {
    assemble(true, true, false, false)
}

pub(super) fn bridge() -> Graph {
    assemble(true, false, false, false)
}

pub(super) fn legacy() -> Graph {
    assemble(false, false, false, false)
}

fn assemble(tombstone: bool, references: bool, anchors: bool, execution: bool) -> Graph {
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
    if execution {
        graph
            .plug::<execution::IssueMember>()
            .plug::<execution::IssueClaim>()
            .plug::<execution::IssueBoundary>()
            .plug::<execution::IssueChange>();
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
    use super::{anchored, bridge, legacy, migration, released};
    use crate::{Coordinate, Seat};
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

    #[tokio::test]
    async fn anchor_compatibility() {
        let temp = tempfile::tempdir().expect("temporary estate");
        let seat = Seat::new(temp.path());
        std::fs::create_dir_all(seat.database().parent().expect("estate root"))
            .expect("estate root");
        let wire = Sqlite::file(seat.database())
            .await
            .expect("anchor database");
        let mut held = keel::bootstrap(anchored(), wire).expect("anchor graph");
        let sudo = held.mint().await.expect("anchor sudo");
        let core = held.seal(&sudo).await.expect("anchor estate");
        core.put("Space", &[("name", "space"), ("revision", "0")])
            .await
            .expect("Space");
        core.put(
            "Anchor",
            &[
                ("node", "I_anchor"),
                ("owner", "PerishLab"),
                ("repository", "concord"),
                ("number", "25"),
                ("revision", "0"),
            ],
        )
        .await
        .expect("Anchor");
        drop(core);
        crate::path::at(&seat.sudo())
            .file(&sudo)
            .expect("sudo file");
        crate::path::at(&seat.database())
            .mode(0o600)
            .expect("database mode");

        let estate = seat.open().await.expect("open exact anchor estate");
        let coordinate = Coordinate::parse("PerishLab/concord#25").expect("coordinate");
        assert_eq!(
            estate.issue(&coordinate).await.expect("Anchor").node,
            "I_anchor"
        );
        assert_eq!(
            estate
                .issue_worktrees()
                .await
                .expect_err("execution resources require explicit transition")
                .code(),
            "concord.issue.execution_migration_required"
        );
    }
}
