mod args;
mod dispatch;
mod output;
mod project;
mod runtime;
mod skill;

pub(crate) use runtime::{config, observation, phase};

use args::Cli;
use clap::Parser;
use std::io::Write;

struct Session(Option<plumb::observation::Run>);

impl Session {
    fn finish(&mut self, code: i32) {
        if let Some(run) = self.0.take() {
            run.finish(code);
        }
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        self.finish(101);
    }
}

fn err(text: impl std::fmt::Display) {
    let _ = writeln!(std::io::stderr().lock(), "{text}");
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    if let Err(error) = plumb::identity!("CONCORD") {
        err(format_args!("concord: {error}"));
        std::process::exit(1);
    }
    let cli = Cli::parse();
    let json = cli.json;
    let mut session = Session(observation::start(
        cli.command.name(),
        cli.config.as_deref(),
        cli.home.as_deref(),
    ));
    let result = match plumb::identity::ready() {
        Ok(()) => dispatch::run(cli).await,
        Err(error) => Err(concord_core::Error::new(error)),
    };
    let code = result.as_ref().map_or(1, |completion| completion.code());
    if let Err(error) = &result {
        observation::fault(error);
    }
    session.finish(code);
    if let Err(error) = result {
        if json {
            err(failure(&error));
        } else {
            err(format_args!("concord: {error}"));
            for fault in faults(&error) {
                err(format_args!("concord:   {fault}"));
            }
            if let Some(code) = output::cookbook::reference(error.code()) {
                err(format_args!("concord: see: concord cookbook {code}"));
            }
        }
    }
    if code != 0 {
        std::process::exit(code);
    }
}

fn faults(error: &concord_core::Error) -> Vec<String> {
    let Some(faults) = error
        .details()
        .and_then(|details| details["agreement"]["faults"].as_array())
    else {
        return Vec::new();
    };
    faults
        .iter()
        .map(|fault| {
            let field = |name: &str| fault[name].as_str().unwrap_or("-").to_string();
            format!(
                "{} {}: {}",
                field("code"),
                field("subject"),
                field("message")
            )
        })
        .collect()
}

fn failure(error: &concord_core::Error) -> String {
    let reference = output::cookbook::reference(error.code());
    let mut body = serde_json::json!({
        "error": {
            "code": error.code(),
            "message": error.message(),
            "details": error.details(),
        }
    });
    if let Some(reference) = reference {
        body["error"]
            .as_object_mut()
            .expect("error body")
            .insert("cookbook".into(), reference.into());
    }
    serde_json::to_string(&body).expect("error JSON should encode")
}

#[cfg(test)]
mod tests {
    use super::{Session, failure};
    use plumb::observation::{Declaration, Run};

    #[test]
    fn references() {
        let complex = concord_core::Error::typed("concord.boundary.refused", "refused");
        let complex: serde_json::Value =
            serde_json::from_str(&failure(&complex)).expect("complex error JSON");
        assert_eq!(complex["error"]["cookbook"], "concord.boundary.refused");

        let simple = concord_core::Error::typed("concord.coordinate.invalid", "invalid");
        let simple: serde_json::Value =
            serde_json::from_str(&failure(&simple)).expect("simple error JSON");
        assert!(simple["error"].get("cookbook").is_none());
    }

    #[test]
    fn unwind() {
        let root = tempfile::tempdir().expect("root");
        let file = root.path().join("config.toml");
        let buffer = root.path().join("locus");
        std::fs::write(
            &file,
            "[locus]\nenabled = true\nendpoint = \"http://127.0.0.1:9\"\n",
        )
        .expect("config");
        let run = Run::start(
            Declaration {
                producer: "concord",
                file: &file,
                buffer: buffer.clone(),
            },
            "cookbook",
        );
        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
            let _session = Session(run);
            panic!("simulated");
        }));
        assert!(outcome.is_err());
        let atoms = std::fs::read_to_string(buffer.join("active.jsonl"))
            .expect("active buffer")
            .lines()
            .map(|line| serde_json::from_str::<serde_json::Value>(line).expect("atom"))
            .collect::<Vec<_>>();
        let finish = atoms
            .iter()
            .find(|atom| atom["payload"]["event"] == "cli.finish")
            .expect("cli.finish on panic");
        assert_eq!(finish["payload"]["code"], 101);
    }
}
