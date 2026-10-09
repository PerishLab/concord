mod args;
mod config;
mod dispatch;
mod observation;
mod output;
mod skill;

use args::Cli;
use clap::Parser;

#[tokio::main(flavor = "current_thread")]
async fn main() {
    if let Err(error) = plumb::identity!("CONCORD") {
        eprintln!("concord: {error}");
        std::process::exit(1);
    }
    let cli = Cli::parse();
    let json = cli.json;
    let observation = observation::Run::start(cli.command.name(), cli.config.as_deref());
    let result = match plumb::identity::ready() {
        Ok(()) => dispatch::run(cli).await,
        Err(error) => Err(concord_core::Error::new(error)),
    };
    if let Some(observation) = observation {
        observation.finish(
            i32::from(result.is_err()),
            result.as_ref().err().map(concord_core::Error::code),
        );
    }
    if let Err(error) = result {
        if json {
            eprintln!("{}", failure(&error));
        } else {
            eprintln!("concord: {error}");
            for fault in faults(&error) {
                eprintln!("concord:   {fault}");
            }
            if let Some(code) = output::cookbook::reference(error.code()) {
                eprintln!("concord: see: concord cookbook {code}");
            }
        }
        std::process::exit(1);
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
    use super::failure;

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
}
