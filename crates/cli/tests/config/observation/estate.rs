use super::super::process;
use super::super::spawn::{Reply, Server};
use super::Observed;
use std::fs;
use std::path::Path;

pub(super) fn parity(configs: &[String]) {
    let estates = configs
        .iter()
        .map(|config| estate(config))
        .collect::<Vec<_>>();
    assert!(
        estates.windows(2).all(|pair| pair[0] == pair[1]),
        "{estates:?}"
    );
}

fn estate(config: &str) -> Vec<(Option<i32>, String)> {
    let home = Observed::new(config);
    let root = home.0.path().join("space");
    fs::create_dir(&root).expect("space root");
    let issue = serde_json::json!({
        "id": "I_parity",
        "number": 26,
        "title": "Parity",
        "body": "",
        "url": "https://github.com/PerishLab/concord/issues/26",
        "state": "OPEN",
        "reason": null,
        "updated": "one",
        "writable": true,
        "kind": {"name": "Feature"},
        "repository": {"id": "R_concord"},
    });
    let repository = serde_json::json!({
        "nameWithOwner": "PerishLab/concord",
        "issue": issue,
    });
    let reply = serde_json::json!({"data": {"repository": repository}});
    let server = Server::start(move |_| Reply::json(reply.clone()));
    let mut seen = Vec::new();
    for arguments in [
        vec!["issue", "bootstrap"],
        vec!["issue", "attach", "PerishLab/concord#26"],
        vec!["issue", "show", "PerishLab/concord#26"],
        vec!["audit"],
    ] {
        let mut command = process(
            home.0.path(),
            &["--home", root.to_str().expect("root"), "--json"],
        );
        server.attach(&mut command);
        let output = command.args(&arguments).output().expect("estate command");
        assert_eq!(
            output.status.code(),
            Some(0),
            "{arguments:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let mut value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        if let Some(estate) = value.get_mut("estate") {
            estate["space"] = "<space>".into();
        }
        if value.get("space").is_some() {
            value["space"] = "<space>".into();
        }
        let encoded = serde_json::to_string(&value).unwrap();
        let stdout = encoded.replace(&home.0.path().display().to_string(), "<home>");
        seen.push((output.status.code(), stdout));
    }
    let workspace = root.join("workspace");
    seen.push((None, inventory(&workspace, &workspace).join("\n")));
    assert_eq!(
        root.join("state/locus/active.jsonl").is_file(),
        config.contains("enabled = true")
    );
    seen
}

fn inventory(root: &Path, directory: &Path) -> Vec<String> {
    let mut files = Vec::new();
    for entry in fs::read_dir(directory).expect("estate directory") {
        let path = entry.expect("estate entry").path();
        if path.is_dir() {
            files.extend(inventory(root, &path));
        } else {
            files.push(
                path.strip_prefix(root)
                    .expect("relative")
                    .display()
                    .to_string(),
            );
        }
    }
    files.sort();
    files
}
