use super::super::process;
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
    let provider = home.0.path().join("provider");
    let reply = serde_json::json!({
        "node": "I_parity",
        "stable": "R_concord",
        "number": 26,
        "url": "https://github.com/PerishLab/concord/issues/26",
        "state": "OPEN",
        "kind": "Feature",
        "updated_at": "one",
    });
    plumb::testing::executable(&provider, format!("#!/bin/sh\nprintf '%s\\n' '{reply}'\n"));
    let provider = provider.to_str().expect("provider path");
    let mut seen = Vec::new();
    for arguments in [
        vec!["issue", "bootstrap"],
        vec![
            "issue",
            "attach",
            "PerishLab/concord#26",
            "--github-command",
            provider,
        ],
        vec!["issue", "show", "PerishLab/concord#26"],
        vec!["audit"],
    ] {
        let output = process(
            home.0.path(),
            &["--root", root.to_str().expect("root"), "--json"],
        )
        .args(&arguments)
        .output()
        .expect("estate command");
        assert_eq!(
            output.status.code(),
            Some(0),
            "{arguments:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let stdout = String::from_utf8_lossy(&output.stdout)
            .replace(&home.0.path().display().to_string(), "<home>");
        seen.push((output.status.code(), stdout));
    }
    seen.push((None, inventory(&root, &root).join("\n")));
    assert_eq!(
        home.buffer().join("active.jsonl").is_file(),
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
