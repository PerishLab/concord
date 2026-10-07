use concord_core::acceptance::{Marker, Target, Verification};

use crate::{closure, comment, declaration, reference};

#[test]
fn roundtrip() {
    let declared = comment("IC_first", &declaration(Target::Source));
    let markers = [
        declaration(Target::Source),
        declaration(Target::Release),
        Marker::Amendment {
            target: Target::Release,
            promise: "Verify the installed consumer after stable distribution.".into(),
            predecessor: reference(&declared),
            reason: "The promise now includes publication.".into(),
        },
        closure(&declared, Target::Source),
    ];
    for marker in markers {
        assert_eq!(
            Marker::parse(&marker.render().unwrap()).unwrap(),
            Some(marker)
        );
    }
}

#[test]
fn ordinary() {
    let execution = "Ordinary progress\n\n<!-- concord.issue-comment/v1\n{\"agent\":\"codex\",\"session\":\"session\",\"host\":null}\n-->";
    assert_eq!(Marker::parse(execution).unwrap(), None);
    let marker = declaration(Target::Source);
    let combined = format!("{}\n\n{execution}", marker.render().unwrap());
    assert_eq!(Marker::parse(&combined).unwrap(), Some(marker));
}

#[test]
fn examples() {
    let marker = declaration(Target::Source).render().unwrap();
    for text in [
        format!("```html\n{marker}\n```"),
        format!("    {}", marker.replace('\n', "\n    ")),
        format!("> {}", marker.replace('\n', "\n> ")),
        format!("`{marker}`"),
    ] {
        assert_eq!(Marker::parse(&text).unwrap(), None, "{text}");
    }
}

#[test]
fn versions() {
    let valid = declaration(Target::Source).render().unwrap();
    for text in [
        valid.replace("/v1", "/v2"),
        valid.replace("/v1", "/v01"),
        valid.replace("v1\n", "v1 "),
        valid.replace("-->", ""),
    ] {
        assert!(Marker::parse(&text).is_err(), "{text}");
    }
}

#[test]
fn fields() {
    let valid = declaration(Target::Source).render().unwrap();
    for text in [
        valid.replace("\"source\"", "\"consumer\""),
        valid.replace("\"declaration\"", "\"unknown\""),
        valid.replace("{\"purpose\"", "{\"unknown\":true,\"purpose\""),
        valid.replace("\"target\":", "\"target\":\"release\",\"target\":"),
        valid.replace("\"promise\"", "\"other\""),
        valid.replace("{", "["),
    ] {
        assert!(Marker::parse(&text).is_err(), "{text}");
    }
}

#[test]
fn ambiguity() {
    let marker = declaration(Target::Source).render().unwrap();
    for text in [
        format!("{marker}\n{marker}"),
        format!("{marker}\n\nordinary text\n\n{marker}"),
        format!("{marker}\n<!-- concord.acceptance/v2\n{{}}\n-->"),
    ] {
        assert!(Marker::parse(&text).is_err());
    }
}

#[test]
fn strength() {
    let declared = comment("IC_first", &declaration(Target::Source));
    let mut marker = closure(&declared, Target::Source);
    let Marker::Closure { verification, .. } = &mut marker else {
        unreachable!()
    };
    *verification = Verification::Machine;
    let text = marker.render().unwrap();
    assert!(text.contains("\"verification\":\"machine\""));
    assert_eq!(Marker::parse(&text).unwrap(), Some(marker));
}

#[test]
fn bounds() {
    assert!(Marker::parse(&"a".repeat(65_537)).is_err());
    let marker = Marker::Declaration {
        target: Target::Source,
        promise: "a".repeat(65_536),
    };
    assert!(marker.render().is_err());
}
