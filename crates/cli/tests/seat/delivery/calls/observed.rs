use super::fixture::Native;
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Output;

#[test]
fn observed() {
    let temp = tempfile::tempdir().expect("fixture");
    let seat = Native::open(temp.path(), "R_kgDOUesM0Q");
    assert!(seat.prepare("wharf-native").status.success());
    let home = seat.space.join("home/.concord");
    assert!(!home.join("state/locus").exists());
    std::fs::write(seat.space.join("fail-gate"), "fail").expect("failure");
    let disabled = seat.prepare("wharf-native");
    failure(&disabled);
    configure(seat.space);
    let enabled = seat.prepare("wharf-native");
    failure(&enabled);
    assert_eq!(disabled.status.code(), enabled.status.code());
    assert_eq!(disabled.stdout, enabled.stdout);
    std::fs::remove_file(seat.space.join("fail-gate")).expect("recover");
    let prepared = seat.prepare("wharf-native");
    assert!(prepared.status.success());
    std::fs::write(seat.space.join("fail-gate"), "fail").expect("failure");
    failure(&seat.land("wharf-native", &prepared.stdout));
    std::fs::remove_file(seat.space.join("fail-gate")).expect("recover");
    let landed = seat.land("wharf-native", &prepared.stdout);
    assert!(
        landed.status.success(),
        "{}",
        String::from_utf8_lossy(&landed.stderr)
    );
    let result: Value = serde_json::from_slice(&landed.stdout).expect("land result");
    assert_eq!(result["merged"], true);
    assert_eq!(result["released"], true);
    inspect(&home.join("state/locus"));
}

#[test]
fn unavailable() {
    let temp = tempfile::tempdir().expect("fixture");
    let seat = Native::open(temp.path(), "R_kgDOUesM0Q");
    let home = configure(seat.space);
    std::fs::create_dir_all(home.join("state")).expect("state");
    std::fs::write(home.join("state/locus"), "blocked").expect("blocked buffer");
    let prepared = seat.prepare("wharf-native");
    assert!(
        prepared.status.success(),
        "{}",
        String::from_utf8_lossy(&prepared.stderr)
    );
    std::fs::write(seat.space.join("fail-gate"), "fail").expect("failure");
    failure(&seat.prepare("wharf-native"));
    assert_eq!(
        std::fs::read_to_string(home.join("state/locus")).expect("buffer obstruction"),
        "blocked"
    );
}

fn failure(output: &Output) {
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8_lossy(&output.stderr);
    let line = stderr.lines().last().expect("business refusal");
    let value: Value = serde_json::from_str(line).expect("JSON refusal after diagnostics");
    assert_eq!(value["error"]["code"], "concord.delivery.native");
}

fn configure(space: &Path) -> PathBuf {
    let home = space.join("home/.concord");
    std::fs::create_dir_all(&home).expect("home");
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("listener");
    let endpoint = format!("http://{}", listener.local_addr().expect("endpoint"));
    drop(listener);
    std::fs::write(
        home.join("concord.toml"),
        format!("[locus]\nenabled = true\nendpoint = \"{endpoint}\"\n"),
    )
    .expect("observation config");
    home
}

fn inspect(buffer: &Path) {
    let mut spans = BTreeMap::<String, Vec<Value>>::new();
    for entry in std::fs::read_dir(buffer).expect("buffer") {
        let path = entry.expect("segment").path();
        if path.extension().is_none_or(|value| value != "jsonl") {
            continue;
        }
        let text = std::fs::read_to_string(path).expect("records");
        assert!(!text.contains("native-test-sentinel"));
        for line in text.lines() {
            let atom: Value = serde_json::from_str(line).expect("Atom");
            let module = atom["source"]["module"].as_str().unwrap_or("");
            if !module.starts_with("concord_core::estate::work::issue::delivery::") {
                continue;
            }
            assert_eq!(atom["producer"], "concord");
            assert!(atom["context"]["concord.command"].is_string());
            assert!(atom.get("payload").is_none());
            let key = atom["context"]["locus.span"]
                .as_str()
                .expect("span")
                .to_string();
            spans.entry(key).or_default().push(atom);
        }
    }
    let mut functions = BTreeMap::<String, usize>::new();
    for atoms in spans.values_mut() {
        atoms.sort_by_key(|atom| atom["at"].as_u64().expect("time"));
        assert_eq!(atoms.len(), 2);
        assert_eq!(atoms[0]["source"]["edge"], "enter");
        assert_eq!(atoms[1]["source"]["edge"], "return");
        assert_eq!(
            atoms[0]["context"]["locus.trace"],
            atoms[1]["context"]["locus.trace"]
        );
        let function = atoms[0]["source"]["function"]
            .as_str()
            .expect("function")
            .to_string();
        *functions.entry(function).or_default() += 1;
    }
    for function in ["prepare", "revalidate", "verify", "run", "suite"] {
        assert!(
            functions.contains_key(function),
            "missing {function}: {functions:?}"
        );
    }
    assert!(functions["verify"] > functions["suite"]);
}
