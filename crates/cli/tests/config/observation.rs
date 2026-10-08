use super::{path, process};
use serde_json::Value;
use std::fs;
use std::process::Output;

struct Observed(tempfile::TempDir);

impl Observed {
    fn new() -> Self {
        Self(tempfile::tempdir().expect("home"))
    }

    fn run(&self, pairs: &[(&str, String)]) -> Output {
        let mut command = process(self.0.path(), &["config", "path"]);
        command.env("CONCORD_LOCUS_ENABLED", "true");
        command.env_remove("CONCORD_LOCUS_REPORT_SPOOL");
        command.envs(pairs.iter().map(|(name, value)| (name, value)));
        command.output().expect("observe concord")
    }
}

#[test]
fn retired() {
    let home = Observed::new();
    let (old, spool) = (home.0.path().join("old.jsonl"), home.0.path().join("spool"));
    let output = home.run(&[
        ("CONCORD_LOCUS_REPORT_FILE", path(&old)),
        ("CONCORD_LOCUS_REPORT_SPOOL", path(&spool)),
    ]);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success() && stderr.contains("CONCORD_LOCUS_REPORT_FILE is retired"));
    assert!(!old.exists() && !spool.exists());
}

#[test]
fn spooled() {
    let home = Observed::new();
    let spool = home.0.path().join("spool");
    let pairs = [
        ("CONCORD_LOCUS_REPORT_SPOOL", path(&spool)),
        ("CONCORD_LOCUS_REPORT_CEILING", "8192".to_string()),
        ("CONCORD_LOCUS_REPORT_SEGMENT", "2048".to_string()),
    ];
    let losses: usize = (0..60)
        .map(|_| home.run(&pairs))
        .inspect(|output| assert!(output.status.success()))
        .map(|output| {
            String::from_utf8_lossy(&output.stderr)
                .matches("reporter.loss")
                .count()
        })
        .sum();
    let segments: Vec<_> = fs::read_dir(&spool)
        .expect("spool")
        .map(|entry| entry.expect("entry").path())
        .filter(|path| {
            let name = path.file_name().unwrap_or_default().to_string_lossy();
            name == "active.jsonl" || name.starts_with("sealed-")
        })
        .collect();
    let bytes: u64 = segments
        .iter()
        .map(|path| fs::metadata(path).expect("size").len())
        .sum();
    assert!(bytes <= 8192 && losses > 0, "{bytes} {losses}");
    assert!(
        !fs::read_to_string(spool.join("loss.jsonl"))
            .expect("ledger")
            .is_empty()
    );
    for line in segments.iter().flat_map(|path| {
        fs::read_to_string(path)
            .expect("segment")
            .lines()
            .map(String::from)
            .collect::<Vec<_>>()
    }) {
        let atom: Value = serde_json::from_str(&line).expect("atom");
        assert!(atom["producer"] == "concord" && atom["id"].is_string());
    }
}
