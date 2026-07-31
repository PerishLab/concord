use locus::{Context, Engine};
use std::sync::OnceLock;

static SEAT: OnceLock<Seat> = OnceLock::new();

struct Seat {
    engine: Engine,
    context: Context,
}

pub fn install(engine: Engine, context: Context) {
    let _ = SEAT.set(Seat { engine, context });
}

pub fn view() -> Option<(&'static Engine, &'static Context)> {
    SEAT.get().map(|seat| (&seat.engine, &seat.context))
}
