use locus::reporter;
use locus::{Candidate, Config, Context, Engine, Key, Policy, Role};
use serde::Deserialize;
use serde_json::json;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

const ENDPOINT: &str = "http://127.0.0.1:43308";

#[derive(Default, Deserialize)]
struct File {
    #[serde(default)]
    locus: Settings,
}

#[derive(Default, Deserialize)]
struct Settings {
    #[serde(default)]
    enabled: bool,
    endpoint: Option<String>,
}

pub(crate) struct Run {
    context: Context,
}

static COMMAND: OnceLock<Context> = OnceLock::new();

impl Run {
    pub(crate) fn start(command: &'static str, config: Option<&Path>) -> Option<Self> {
        let (engine, context) = load(config)?;
        let candidate = Candidate::event(json!({"event": "cli.start"}))
            .ensure(Role::trace())
            .ensure(Role::span())
            .explicit(Role::new("concord.command").ok()?, Key::new(command).ok()?);
        let cycle = engine.append(&context, candidate).ok()?.context();
        let _ = COMMAND.set(cycle.clone());
        concord_core::observation::install(engine, context);
        Some(Self { context: cycle })
    }

    pub(crate) fn finish(self, code: i32, fault: Option<&str>) {
        let Some((engine, _)) = concord_core::observation::view() else {
            return;
        };
        let mut candidate = Candidate::event(json!({
            "event": "cli.finish",
            "code": code,
        }))
        .ensure(Role::trace())
        .ensure(Role::span());
        if let Some(fault) = fault
            && let (Ok(role), Ok(key)) = (Role::new("concord.fault"), Key::new(fault))
        {
            candidate = candidate.explicit(role, key);
        }
        let _ = engine.append(&self.context, candidate);
    }
}

pub(crate) fn projection(shape: &str, provider: (&str, &str), duration: u64, outcome: &str) {
    let Some(context) = COMMAND.get() else {
        return;
    };
    let Some((engine, _)) = concord_core::observation::view() else {
        return;
    };
    let candidate = Candidate::event(json!({
        "event": "provider.projection",
        "shape": shape,
        "provider": provider.0,
        "kind": provider.1,
        "duration_ms": duration,
        "outcome": outcome,
    }))
    .ensure(Role::trace())
    .ensure(Role::span());
    let _ = engine.append(context, candidate);
}

fn load(config: Option<&Path>) -> Option<(Engine, Context)> {
    if let Some(name) = concord_core::retired("CONCORD_LOCUS_") {
        eprintln!("concord locus config: {name} is retired; configure [locus] in concord.toml");
        return None;
    }
    let path = crate::config::Config::path(config).ok()?;
    if !path.is_file() {
        return None;
    }
    let file: File = match plumb::config::load(&path) {
        Ok(file) => file,
        Err(error) => {
            eprintln!("concord locus config: {error}");
            return None;
        }
    };
    if !file.locus.enabled {
        return None;
    }
    let endpoint = file
        .locus
        .endpoint
        .or_else(|| plumb::config::value("LOCUS_API"))
        .unwrap_or_else(|| ENDPOINT.to_string());
    let buffer = plumb::config::data("concord")?.join("state").join("locus");
    match build(endpoint, buffer) {
        Ok(run) => Some(run),
        Err(error) => {
            eprintln!("concord locus: {error}");
            None
        }
    }
}

fn build(endpoint: String, buffer: PathBuf) -> Result<(Engine, Context), locus::Error> {
    let policy = Policy::default()
        .producer("concord")
        .reporter(reporter::Spec::api(endpoint, buffer));
    let engine = Engine::bootstrap(Config::new(policy))?;
    let mut candidate = Candidate::context().ensure(Role::trace());
    if let Some(session) = concord_core::session() {
        candidate = candidate.explicit(Role::trace(), Key::new(session)?);
    }
    let context = engine.append(&Context::empty(), candidate)?.context();
    Ok((engine, context))
}
