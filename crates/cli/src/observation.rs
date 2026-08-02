use locus::collector;
use locus::generator;
use locus::reporter;
use locus::{Candidate, Config, Context, Engine, Key, Policy, Role};
use plumb::config::Cascade;
use serde_json::json;
use std::path::PathBuf;

#[derive(Debug, Default, PartialEq, Cascade)]
struct Settings {
    enabled: bool,
    #[cascade(section)]
    report: Report,
    #[cascade(section)]
    trace: Trace,
}

#[derive(Debug, Default, PartialEq, Cascade)]
#[cascade(section)]
struct Report {
    file: PathBuf,
}

#[derive(Debug, Default, PartialEq, Cascade)]
#[cascade(section)]
struct Trace {
    file: PathBuf,
    id: String,
}

pub(crate) struct Run {
    command: &'static str,
    context: Context,
}

impl Run {
    pub(crate) fn start(command: &'static str) -> Option<Self> {
        let (engine, context) = load()?;
        let candidate = Candidate::event(json!({
            "event": "cli.start",
            "command": command,
        }))
        .ensure(Role::trace())
        .ensure(Role::span());
        let cycle = engine.append(&context, candidate).ok()?.context();
        concord_core::observation::install(engine, context);
        Some(Self {
            command,
            context: cycle,
        })
    }

    pub(crate) fn finish(self, code: i32) {
        let Some((engine, _)) = concord_core::observation::view() else {
            return;
        };
        let candidate = Candidate::event(json!({
            "event": "cli.finish",
            "command": self.command,
            "code": code,
        }))
        .ensure(Role::trace())
        .ensure(Role::span());
        let _ = engine.append(&self.context, candidate);
    }
}

fn load() -> Option<(Engine, Context)> {
    let seen = match <Settings as Cascade>::env("CONCORD_LOCUS") {
        Ok(seen) => seen,
        Err(error) => {
            eprintln!("concord locus config: {error}");
            return None;
        }
    };
    let settings = Settings::default().merge(seen);
    if !settings.enabled {
        return None;
    }
    if settings.report.file.as_os_str().is_empty() {
        eprintln!(
            "concord locus config: CONCORD_LOCUS_REPORT_FILE is required when observation is enabled"
        );
        return None;
    }
    match build(settings) {
        Ok(run) => Some(run),
        Err(error) => {
            eprintln!("concord locus: {error}");
            None
        }
    }
}

fn build(settings: Settings) -> Result<(Engine, Context), locus::Error> {
    let trace = Role::trace();
    let mut policy = Policy::default()
        .collector(
            "codex.thread",
            collector::Spec::environment("CODEX_THREAD_ID", 512),
        )
        .reporter(reporter::Spec::file(settings.report.file));
    if !settings.trace.file.as_os_str().is_empty() {
        policy = policy.generator(trace.clone(), generator::Spec::shared(settings.trace.file));
    }
    let engine = Engine::bootstrap(Config::new(policy))?;
    let mut candidate = Candidate::context()
        .collect(trace.clone(), "codex.thread")
        .ensure(trace.clone());
    if !settings.trace.id.is_empty() {
        candidate = candidate.explicit(trace, Key::new(settings.trace.id)?);
    }
    let context = engine.append(&Context::empty(), candidate)?.context();
    Ok((engine, context))
}
