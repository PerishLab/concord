use super::{repository, success, tool};
use serde_json::json;

#[test]
fn ceremony() {
    let fixture = tempfile::tempdir().unwrap();
    let home = fixture.path();
    success(home, &["issue", "bootstrap"]);
    let source = repository(home, "PerishLab/concord");
    let provider = tool(
        home,
        "repository",
        format!(
            "printf '%s\\n' '{}'",
            json!({
                "node": "R_concord", "coordinate": "PerishLab/concord", "branch": "main",
            })
        ),
    );
    let provider = provider.to_str().unwrap();
    success(
        home,
        &[
            "integration",
            "register",
            "PerishLab/concord",
            "--path",
            source.to_str().unwrap(),
            "--github-command",
            provider,
        ],
    );
    let target = home.join("relocated");
    std::fs::rename(&source, &target).unwrap();
    let prepared = success(
        home,
        &[
            "integration",
            "rebind",
            "prepare",
            "PerishLab/concord",
            "--node",
            "R_concord",
            "--key",
            "1",
            "--path",
            target.to_str().unwrap(),
            "--github-command",
            provider,
        ],
    );
    let input = home.join("plan.json");
    std::fs::write(&input, serde_json::to_vec(&prepared).unwrap()).unwrap();
    let mut args = vec![
        "integration",
        "rebind",
        "apply",
        "PerishLab/concord",
        "--input",
        input.to_str().unwrap(),
        "--github-command",
        provider,
    ];
    assert_eq!(success(home, &args)["rebind"]["applied"], false);
    assert_eq!(
        success(home, &["integration", "list"])["integrations"][0]["path"],
        prepared["plan"]["integration"]["path"]
    );
    args.push("--apply");
    let applied = success(home, &args);
    assert_eq!(applied["rebind"]["applied"], true);
    assert_eq!(applied["rebind"]["integration"]["key"], 1);
    assert_eq!(success(home, &["audit"])["agreement"]["faults"], json!([]));
    assert!(target.join(".git").is_dir());
}
