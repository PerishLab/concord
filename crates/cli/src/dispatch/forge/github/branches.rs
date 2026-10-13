use super::{Observed, Reason, Verdict};
use std::collections::BTreeSet;

#[test]
fn judge() {
    let live = BTreeSet::from([
        "concord/issue-abc".to_string(),
        "land/concord/issue-abc".to_string(),
    ]);
    for branch in [
        "main",
        "release/v0.22.0",
        "concord/issue-abc",
        "land/concord/issue-abc",
    ] {
        assert_eq!(
            super::judge(branch, "main", &live),
            Verdict::Held,
            "{branch}"
        );
    }
    assert_eq!(
        super::judge("task/12", "main", &live),
        Verdict::Issue {
            kind: "Task",
            number: 12
        }
    );
    for branch in [
        "topic",
        "fix/12",
        "task/012",
        "task/0",
        "task/12/more",
        "release/v1.2",
        "release/v01.2.3",
        "concord/issue-def",
    ] {
        assert_eq!(
            super::judge(branch, "main", &live),
            Verdict::Refused(Reason::Name),
            "{branch}"
        );
    }
    assert_eq!(
        super::judge("land/task/12", "main", &live),
        Verdict::Projection {
            kind: "Task",
            number: 12
        }
    );
}

#[test]
fn settle() {
    let open = |kind: &str| Observed::Issue {
        open: true,
        kind: Some(kind.to_string()),
    };
    assert_eq!(super::settle("Task", Some(&open("Task"))), None);
    assert_eq!(
        super::settle("Task", Some(&open("Bug"))),
        Some(Reason::Kind)
    );
    assert_eq!(
        super::settle(
            "Task",
            Some(&Observed::Issue {
                open: false,
                kind: Some("Task".into())
            })
        ),
        Some(Reason::Closed)
    );
    assert_eq!(
        super::settle("Task", Some(&Observed::Absent)),
        Some(Reason::Absent)
    );
}
