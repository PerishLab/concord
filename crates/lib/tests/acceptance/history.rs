use concord_core::acceptance::{Comment, History, Judgment, Marker, Target};

use crate::{closure, comment, declaration, reference};

#[test]
fn labels() {
    let labels = vec!["bug".into(), "acceptance:source".into(), "custom".into()];
    assert_eq!(Target::select(&labels).unwrap(), Some(Target::Source));
    assert_eq!(
        Target::Release.replace(&labels).unwrap(),
        vec!["bug", "custom", "acceptance:release"]
    );
    assert_eq!(Target::select(&[]).unwrap(), None);
    assert_eq!(
        Target::Source.replace(&[]).unwrap(),
        vec!["acceptance:source"]
    );
}

#[test]
fn conflicts() {
    for labels in [
        vec!["acceptance:source".into(), "acceptance:release".into()],
        vec!["acceptance:source".into(), "acceptance:source".into()],
        vec!["acceptance:consumer".into()],
    ] {
        assert!(Target::select(&labels).is_err());
        assert!(Target::Source.replace(&labels).is_err());
    }
}

#[test]
fn chain() {
    let first = comment("IC_first", &declaration(Target::Source));
    let closed = comment("IC_closed", &closure(&first, Target::Source));
    let mut comments = vec![first.clone(), closed];
    let history = History::read(&comments).unwrap();
    assert_eq!(
        history.declaration.as_ref().unwrap().reference,
        reference(&first)
    );
    assert!(history.closure.is_some());
    let amended = comment("IC_amended", &amendment(&first));
    comments.push(amended.clone());
    let history = History::read(&comments).unwrap();
    assert_eq!(
        history.declaration.as_ref().unwrap().reference,
        reference(&amended)
    );
    assert!(history.closure.is_none());
    assert_eq!(
        history.target(&["acceptance:release".into()]).unwrap(),
        Some(Target::Release)
    );
    assert!(history.target(&["acceptance:source".into()]).is_err());
}

#[test]
fn partial() {
    let first = comment("IC_first", &declaration(Target::Source));
    let empty = History::read(&[]).unwrap();
    assert_eq!(empty.target(&[]).unwrap(), None);
    assert!(empty.target(&["acceptance:source".into()]).is_err());
    let history = History::read(&[first]).unwrap();
    assert!(history.target(&[]).is_err());
    assert_eq!(
        history.target(&["acceptance:source".into()]).unwrap(),
        Some(Target::Source)
    );
}

#[test]
fn stale() {
    let first = comment("IC_first", &declaration(Target::Source));
    let amended = comment("IC_amended", &amendment(&first));
    let closed = comment("IC_closed", &closure(&first, Target::Source));
    assert!(History::read(&[first.clone(), amended.clone(), closed]).is_err());
    assert!(
        History::read(&[
            first.clone(),
            amended,
            comment("IC_other", &amendment(&first))
        ])
        .is_err()
    );
    let mut edited = first.clone();
    edited.body.push_str("\nThe declaration was edited.");
    assert!(
        History::read(&[
            edited,
            comment("IC_closed", &closure(&first, Target::Source))
        ])
        .is_err()
    );
}

#[test]
fn identity() {
    let first = comment("IC_first", &declaration(Target::Source));
    assert!(History::read(&[first.clone(), first.clone()]).is_err());
    assert!(History::read(&[first, comment("IC_other", &declaration(Target::Source))]).is_err());
    assert!(
        History::read(&[Comment {
            node: "".into(),
            body: "ordinary".into()
        }])
        .is_err()
    );
}

#[test]
fn predecessor() {
    let first = comment("IC_first", &declaration(Target::Source));
    assert!(History::read(&[comment("IC_amended", &amendment(&first))]).is_err());
    assert!(History::read(&[comment("IC_closed", &closure(&first, Target::Source))]).is_err());
    let mut mismatched = closure(&first, Target::Release);
    let Marker::Closure {
        judgment,
        remaining,
        ..
    } = &mut mismatched
    else {
        unreachable!()
    };
    *judgment = Judgment::Unmet;
    remaining.push("Publication remains unverified.".into());
    assert!(History::read(&[first, comment("IC_closed", &mismatched)]).is_err());
}

#[test]
fn ordinary() {
    let history = History::read(&[Comment {
        node: "IC_progress".into(),
        body: "Checks passed; source delivery remains pending.".into(),
    }])
    .unwrap();
    assert_eq!(history, History::default());
}

fn amendment(first: &Comment) -> Marker {
    Marker::Amendment {
        target: Target::Release,
        promise: "Verify the published capability in its installed consumer.".into(),
        predecessor: reference(first),
        reason: "The endpoint now includes publication.".into(),
    }
}
