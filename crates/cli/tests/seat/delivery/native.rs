use super::super::execution::unix::{git, text};
use super::fixture::Native;
use super::unix::{refused, success};
use serde_json::{Value, json};

#[test]
fn native() {
    let temp = tempfile::tempdir().expect("fixture");
    let seat = Native::open(temp.path(), "R_kgDOUesM0Q");
    refused(&seat.prepare("plumb"), "concord.delivery.authority");
    let prepared = seat.prepare("wharf-native");
    assert!(
        prepared.status.success(),
        "{}",
        String::from_utf8_lossy(&prepared.stderr)
    );
    let mut envelope: Value = serde_json::from_slice(&prepared.stdout).expect("plan");
    assert_eq!(envelope["plan"]["authority"]["kind"], "wharf-native");
    let candidate = envelope["plan"]["delivery"]["candidate"]
        .as_str()
        .expect("candidate");
    let message = text(&seat.member, &["log", "-1", "--format=%B", candidate]);
    assert!(message.contains("Native-Gate-Proof:"));
    assert!(!message.contains(plumb::guard::TRAILER));
    refused(
        &seat.land("plumb", &prepared.stdout),
        "concord.delivery.authority",
    );
    std::fs::write(seat.space.join("fail-gate"), "fail").expect("failure");
    refused(
        &seat.land("wharf-native", &prepared.stdout),
        "concord.delivery.native",
    );
    assert!(!seat.space.join("pull-state").exists());
    std::fs::remove_file(seat.space.join("fail-gate")).expect("recover gate");
    envelope["plan"]["authority"]["evidence"]["digest"] = json!("0".repeat(64));
    refused(
        &seat.land(
            "wharf-native",
            &serde_json::to_vec(&envelope).expect("tamper"),
        ),
        "concord.delivery.stale",
    );
    assert!(!seat.space.join("pull-state").exists());
    let labels = json!({"nodes": [{"name": "needs:owner"}], "pageInfo": {"hasNextPage": false}});
    std::fs::write(seat.space.join("labels"), labels.to_string()).expect("needs");
    refused(
        &seat.land("wharf-native", &prepared.stdout),
        "concord.issue.needs",
    );
    std::fs::remove_file(seat.space.join("labels")).expect("clear needs");
    let landed = seat.land("wharf-native", &prepared.stdout);
    assert!(
        landed.status.success(),
        "{}",
        String::from_utf8_lossy(&landed.stderr)
    );
    let report: Value = serde_json::from_slice(&landed.stdout).expect("report");
    assert_eq!(report["merged"], true);
    assert_eq!(report["released"], true);
    let status = std::fs::read_to_string(seat.space.join("status-call")).expect("native status");
    assert!(status.contains("context=native / wharf (pull_request)"));
    assert!(!status.contains("context=guard"));
    assert!(!seat.member.exists());
    let head = text(&seat.source, &["rev-parse", "HEAD"]);
    assert_eq!(head, report["merge"]);
    assert!(text(&seat.source, &["log", "-1", "--format=%B"]).contains("Native-Gate-Proof:"));
    assert!(seat.land("wharf-native", &prepared.stdout).status.success());
}

#[test]
fn identity() {
    let temp = tempfile::tempdir().expect("fixture");
    let seat = Native::open(temp.path(), "R_other");
    refused(&seat.prepare("wharf-native"), "concord.delivery.authority");
    assert!(!seat.space.join("pull-state").exists());
}

#[test]
fn readback() {
    let temp = tempfile::tempdir().expect("fixture");
    let seat = Native::open(temp.path(), "R_kgDOUesM0Q");
    let prepared = seat.prepare("wharf-native");
    assert!(prepared.status.success());
    let before = text(&seat.source, &["rev-parse", "HEAD"]);
    std::fs::write(seat.space.join("lose-proof"), "fail").expect("readback flag");
    refused(
        &seat.land("wharf-native", &prepared.stdout),
        "concord.delivery.landed",
    );
    assert!(seat.member.exists());
    assert_eq!(text(&seat.source, &["rev-parse", "HEAD"]), before);
}

#[test]
fn governed() {
    let temp = tempfile::tempdir().expect("fixture");
    let seat = Native::open(temp.path(), "R_kgDOUesM0Q");
    std::fs::write(seat.member.join("plumb.toml"), "guard = true\n").expect("governed");
    git(&seat.member, &["add", "plumb.toml"]);
    git(&seat.member, &["commit", "-m", "declare governance"]);
    success(
        seat.space,
        &[
            "member",
            "claim",
            "PerishLab/wharf#1",
            "--revision",
            "2",
            "--claim",
            "topic.md",
            "--claim",
            "plumb.toml",
        ],
    );
    success(
        seat.space,
        &["member", "prove", "PerishLab/wharf#1", "--revision", "3"],
    );
    let shown = success(seat.space, &["issue", "show", "PerishLab/wharf#1"]);
    assert_eq!(shown["anchor"]["revision"], 4);
    refused(&seat.prepare("wharf-native"), "concord.delivery.native");
}
