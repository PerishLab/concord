use super::{Reply, Server, record, repository, success};
use serde_json::json;

#[test]
fn ceremony() {
    let fixture = tempfile::tempdir().unwrap();
    let home = fixture.path();
    success(home, None, &["issue", "bootstrap"]);
    let source = repository(home, "PerishLab/concord");
    let server = Server::start(|_| Reply::json(record("R_concord", "PerishLab/concord")));
    success(
        home,
        Some(&server),
        &[
            "integration",
            "register",
            "PerishLab/concord",
            "--path",
            source.to_str().unwrap(),
        ],
    );
    let target = home.join("relocated");
    std::fs::rename(&source, &target).unwrap();
    let prepared = success(
        home,
        Some(&server),
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
    ];
    assert_eq!(
        success(home, Some(&server), &args)["rebind"]["applied"],
        false
    );
    assert_eq!(
        success(home, None, &["integration", "list"])["integrations"][0]["path"],
        prepared["plan"]["integration"]["path"]
    );
    args.push("--apply");
    let applied = success(home, Some(&server), &args);
    assert_eq!(applied["rebind"]["applied"], true);
    assert_eq!(applied["rebind"]["integration"]["key"], 1);
    assert_eq!(
        success(home, None, &["audit"])["agreement"]["faults"],
        json!([])
    );
    assert!(target.join(".git").is_dir());
}
