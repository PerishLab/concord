use super::{Observed, closed, expected, process, serve};
use serde_json::Value;

#[test]
fn invocation() {
    for unavailable in [false, true] {
        for (args, operation, code) in [
            (vec!["config", "path"], "config.path", 0),
            (vec!["--json", "issue", "show", "invalid"], "issue.show", 1),
        ] {
            let home = Observed::new("");
            let baseline = process(home.0.path(), &args).output().unwrap();
            assert_eq!(baseline.status.code(), Some(code));
            assert!(!home.buffer().exists());
            let endpoint = if unavailable { closed() } else { serve() };
            std::fs::write(
                expected(home.0.path()),
                format!("[locus]\nenabled = true\nendpoint = '{endpoint}'\n"),
            )
            .unwrap();
            let output = process(home.0.path(), &args).output().unwrap();
            assert_eq!(output.status.code(), baseline.status.code());
            assert_eq!(output.stdout, baseline.stdout);
            assert!(output.stderr.ends_with(&baseline.stderr));
            if !unavailable {
                assert_eq!(output.stderr, baseline.stderr);
            }
            verify(&home.atoms(), operation);
        }
    }
}

fn verify(atoms: &[Value], operation: &str) {
    let entries = atoms
        .iter()
        .filter(|atom| {
            atom["source"]["module"] == "concord::dispatch" && atom["source"]["function"] == "run"
        })
        .collect::<Vec<_>>();
    assert_eq!(entries.len(), 2, "{atoms:?}");
    let enter = entries[0];
    let returned = entries[1];
    assert_eq!(enter["source"]["edge"], "enter");
    assert_eq!(returned["source"]["edge"], "return");
    for role in ["locus.span", "locus.trace"] {
        assert!(enter["context"][role].as_str().is_some());
        assert_eq!(enter["context"][role], returned["context"][role]);
    }
    for entry in &entries {
        assert_eq!(entry["context"]["concord.command"], operation);
    }
    let start = atoms
        .iter()
        .find(|atom| atom["payload"]["event"] == "cli.start")
        .unwrap();
    let finish = atoms
        .iter()
        .find(|atom| atom["payload"]["event"] == "cli.finish")
        .unwrap();
    let times = [start, enter, returned, finish].map(|atom| atom["at"].as_u64().unwrap());
    assert!(times.windows(2).all(|pair| pair[0] <= pair[1]));
}
