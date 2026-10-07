use super::{evaluate, world};
use concord_core::acceptance::{
    Judgment, Marker, Reference, Release, Target, Verification, digest,
};
use serde_json::{Value, json};
use std::process::Output;

const AUTHORITY: &str = "https://releases.example.org";

struct World {
    source: evaluate::World,
    manifest: Value,
}

impl World {
    fn new(channel: &str, marker: &str) -> Self {
        let mut source = evaluate::World::new();
        let declaration = Marker::Declaration {
            target: Target::Release,
            promise: "Source and containing stable distribution, manually reviewed".into(),
        }
        .render()
        .unwrap();
        let closure = Marker::Closure {
            target: Target::Release,
            declaration: Reference {
                node: "IC_decl".into(),
                digest: digest(&declaration),
            },
            judgment: Judgment::Satisfied,
            verification: Verification::Manual,
            review: digest(
                source.page["data"]["repository"]["issue"]["body"]
                    .as_str()
                    .unwrap(),
            ),
            evidence: vec!["reviewed source".into()],
            remaining: Vec::new(),
            release: Some(Release {
                marker: "v1.0.0".into(),
                distribution: format!(
                    "{AUTHORITY}/v1/releases/{channel}/{marker}/distribution.json"
                ),
                inclusion: vec!["manually reviewed inclusion".into()],
            }),
        }
        .render()
        .unwrap();
        let issue = &mut source.page["data"]["repository"]["issue"];
        issue["labels"] = world::connection(vec![json!({"name": "acceptance:release"})], None, 1);
        issue["comments"] = world::connection(
            vec![
                world::entry("IC_decl", &declaration, 1),
                world::entry("IC_close", &closure, 2),
            ],
            None,
            2,
        );
        source.fresh["data"]["declaration"]["body"] = declaration.into();
        source.fresh["data"]["closure"]["body"] = closure.into();
        let blob = json!({
            "oid": "a".repeat(40), "text": format!("[release]\nauthority = \"{AUTHORITY}\"\n"),
        });
        let manifest = json!({"data": {"repository": {"id": "R_concord", "object": blob}}});
        Self { source, manifest }
    }

    fn run(&self) -> Output {
        let scratch = tempfile::tempdir().unwrap();
        let mut replies = vec![
            self.source.page.clone(),
            self.source.page.clone(),
            self.source.relations.clone(),
            self.source.relations.clone(),
            self.source.fresh.clone(),
            self.source.fresh.clone(),
            self.source.page.clone(),
            self.source.page.clone(),
            self.source.relations.clone(),
        ];
        let closure = Marker::parse(
            self.source.fresh["data"]["closure"]["body"]
                .as_str()
                .unwrap(),
        )
        .unwrap()
        .unwrap();
        if matches!(
            closure,
            Marker::Closure {
                release: Some(_),
                ..
            }
        ) {
            replies.push(self.manifest.clone());
        }
        replies.extend([self.source.page.clone(), self.source.page.clone()]);
        let provider = world::provider(scratch.path(), &replies);
        let output = world::invoke(scratch.path(), &provider, "evaluate", None);
        assert!(!scratch.path().join(".concord").exists());
        output
    }

    fn check(&self, verdict: &str, reason: &str) {
        let decoded = world::decoded(&self.run());
        let evaluation = &decoded["acceptance"]["evaluation"];
        assert_eq!(evaluation["verdict"], verdict);
        assert_eq!(evaluation["verification"], "manual");
        let release = evaluation["checks"]
            .as_array()
            .unwrap()
            .iter()
            .find(|check| check["name"] == "release")
            .unwrap();
        assert_eq!(release["fact"]["verdict"], verdict);
        assert!(release["fact"]["reason"].as_str().unwrap().contains(reason));
    }
}

#[test]
fn undeclared() {
    let mut held = World::new("stable", "v1.0.0");
    held.manifest["data"]["repository"]["object"] = Value::Null;
    held.check("unknown", "declares no release authority");
}

#[test]
fn foreign() {
    let mut held = World::new("stable", "v1.0.0");
    held.manifest["data"]["repository"]["object"]["text"] =
        "[release]\nauthority = \"https://foreign.example.org\"\n".into();
    held.check("unmet", "outside");
}

#[test]
fn candidate() {
    World::new("rc", "v1.0.0").check("unmet", "exact stable");
}

#[test]
fn conflicting() {
    World::new("stable", "v2.0.0").check("unmet", "exact stable");
}

#[test]
fn identity() {
    let mut held = World::new("stable", "v1.0.0");
    held.manifest["data"]["repository"]["id"] = "R_foreign".into();
    world::refused(&held.run(), "concord.acceptance.identity");
}

#[test]
fn object() {
    let mut held = World::new("stable", "v1.0.0");
    held.manifest["data"]["repository"]["object"]["oid"] = "".into();
    world::refused(&held.run(), "concord.acceptance.identity");
}

#[test]
fn provider() {
    let mut held = World::new("stable", "v1.0.0");
    held.manifest["errors"] = json!([{"message": "denied"}]);
    world::refused(&held.run(), "concord.acceptance.provider");
}

#[test]
fn malformed() {
    let mut held = World::new("stable", "v1.0.0");
    held.manifest["data"]["repository"]["object"]["text"] = "[release".into();
    world::refused(&held.run(), "concord.acceptance.declaration");
}

#[test]
fn old() {
    let mut held = World::new("stable", "v1.0.0");
    let mut marker = Marker::parse(
        held.source.fresh["data"]["closure"]["body"]
            .as_str()
            .unwrap(),
    )
    .unwrap()
    .unwrap();
    if let Marker::Closure {
        judgment,
        remaining,
        release,
        ..
    } = &mut marker
    {
        *judgment = Judgment::Unmet;
        *remaining = vec!["#86-style review: installed stable predates the source merge and lacks the promised command".into()];
        *release = None;
    }
    let body = marker.render().unwrap();
    held.source.fresh["data"]["closure"]["body"] = body.clone().into();
    held.source.page["data"]["repository"]["issue"]["comments"]["nodes"][1]["body"] = body.into();
    let decoded = world::decoded(&held.run());
    let evaluation = &decoded["acceptance"]["evaluation"];
    assert_eq!(evaluation["verdict"], "unmet");
    assert_eq!(evaluation["verification"], "manual");
    let checks = evaluation["checks"].as_array().unwrap();
    assert!(checks.iter().any(|check| {
        check["name"] == "judgment"
            && check["fact"]["verdict"] == "unmet"
            && check["fact"]["reason"]
                .as_str()
                .unwrap()
                .contains("predates the source merge")
    }));
    assert!(
        checks
            .iter()
            .any(|check| check["name"] == "release" && check["fact"]["verdict"] == "unknown")
    );
}
