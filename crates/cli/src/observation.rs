use locus::{Candidate, Role};
use plumb::observation::{Declaration, Run};
use serde_json::{Value, json};
use std::path::Path;
use std::time::{Duration, Instant};

pub(crate) fn start(command: &'static str, config: Option<&Path>) -> Option<Run> {
    let file = crate::config::Config::path(config).ok()?;
    let buffer = plumb::config::data("concord")?.join("state").join("locus");
    Run::start(
        Declaration {
            producer: "concord",
            file: &file,
            buffer,
        },
        command,
    )
}

pub(crate) fn fault(error: &concord_core::Error) {
    plumb::observation::fault(Box::leak(error.code().to_owned().into_boxed_str()));
}

pub(crate) fn projection(shape: &str, provider: (&str, &str), duration: u64, outcome: &str) {
    emit(json!({
        "event": "provider.projection",
        "shape": shape,
        "provider": provider.0,
        "kind": provider.1,
        "duration_ms": duration,
        "outcome": outcome,
    }));
}

pub(crate) fn call(operation: &str, site: &str, elapsed: Duration, outcome: &str) {
    emit(json!({
        "event": "provider.call",
        "provider": "github",
        "operation": operation,
        "site": site,
        "duration_ms": millis(elapsed),
        "outcome": outcome,
    }));
}

pub(crate) async fn wait(reason: &str, delay: Duration) {
    let start = Instant::now();
    tokio::time::sleep(delay).await;
    emit(json!({
        "event": "provider.wait",
        "provider": "github",
        "reason": reason,
        "duration_ms": millis(start.elapsed()),
    }));
}

fn millis(elapsed: Duration) -> u64 {
    u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX)
}

fn emit(payload: Value) {
    let Some((engine, context)) = plumb::observation::view() else {
        return;
    };
    let candidate = Candidate::event(payload)
        .ensure(Role::trace())
        .ensure(Role::span());
    let _ = engine.append(context, candidate);
}
