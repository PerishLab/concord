use concord_core::acceptance::{Judgment, Marker, Release, Target};

use crate::{closure, comment, declaration, reference};

#[test]
fn promises() {
    for promise in ["", " ", "terminal --> injection"] {
        assert!(
            Marker::Declaration {
                target: Target::Source,
                promise: promise.into()
            }
            .render()
            .is_err()
        );
    }
    let first = comment("IC_first", &declaration(Target::Source));
    assert!(
        Marker::Amendment {
            target: Target::Release,
            promise: "Publish".into(),
            predecessor: reference(&first),
            reason: "".into()
        }
        .render()
        .is_err()
    );
}

#[test]
fn judgment() {
    let first = comment("IC_first", &declaration(Target::Source));
    let mut marker = closure(&first, Target::Source);
    let Marker::Closure { evidence, .. } = &mut marker else {
        unreachable!()
    };
    evidence.clear();
    assert!(marker.render().is_err());
    let Marker::Closure {
        judgment,
        remaining,
        ..
    } = &mut marker
    else {
        unreachable!()
    };
    *judgment = Judgment::Unmet;
    assert!(remaining.is_empty());
    assert!(marker.render().is_err());
    let Marker::Closure { remaining, .. } = &mut marker else {
        unreachable!()
    };
    remaining.push("Source merge is outstanding.".into());
    assert!(marker.render().is_ok());
    let Marker::Closure { judgment, .. } = &mut marker else {
        unreachable!()
    };
    *judgment = Judgment::Satisfied;
    assert!(marker.render().is_err());
}

#[test]
fn release() {
    let first = comment("IC_first", &declaration(Target::Release));
    let mut marker = closure(&first, Target::Release);
    assert!(marker.render().is_err());
    let Marker::Closure { release, .. } = &mut marker else {
        unreachable!()
    };
    *release = Some(Release {
        marker: "v1.0.0@exactcommit".into(),
        distribution: "https://releases.example/record/v1.0.0".into(),
        inclusion: vec!["Manual ancestry and installed consumer verification.".into()],
    });
    assert!(marker.render().is_ok());
    let Marker::Closure { target, .. } = &mut marker else {
        unreachable!()
    };
    *target = Target::Source;
    assert!(marker.render().is_err());
}

#[test]
fn references() {
    let first = comment("IC_first", &declaration(Target::Source));
    for value in ["", "deadbeef", &"A".repeat(64)] {
        let mut marker = closure(&first, Target::Source);
        let Marker::Closure { review, .. } = &mut marker else {
            unreachable!()
        };
        *review = value.into();
        assert!(marker.render().is_err());
        let Marker::Closure {
            review,
            declaration,
            ..
        } = &mut marker
        else {
            unreachable!()
        };
        *review = concord_core::acceptance::digest("review");
        declaration.digest = value.into();
        assert!(marker.render().is_err());
    }
}

#[test]
fn entries() {
    let first = comment("IC_first", &declaration(Target::Source));
    for values in [vec!["".into()], vec!["evidence".into(), "evidence".into()]] {
        let mut marker = closure(&first, Target::Source);
        let Marker::Closure { evidence, .. } = &mut marker else {
            unreachable!()
        };
        *evidence = values;
        assert!(marker.render().is_err());
    }
}
