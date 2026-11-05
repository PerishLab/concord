use locus::{Context, Engine};
use radicula_context::Context as Ambient;

#[cfg(test)]
#[path = "observation/proof.rs"]
mod proof;

type Seat = (&'static Engine, Context);

struct Fork(Option<Context>);

pub(crate) fn view() -> Option<Seat> {
    let (engine, root) = plumb::observation::view()?;
    Some((engine, select(root)))
}

fn select(root: &Context) -> Context {
    Ambient::current()
        .get::<Fork>()
        .and_then(|fork| fork.0.clone())
        .unwrap_or_else(|| root.clone())
}

pub(crate) fn borrow(seat: &Seat) -> (&'static Engine, &Context) {
    (seat.0, &seat.1)
}

pub(crate) fn fork(frame: Option<&Context>) -> Option<Context> {
    let (engine, _) = plumb::observation::view()?;
    engine.fork(frame?).ok()
}

pub(crate) fn within<T>(context: Option<Context>, work: impl FnOnce() -> T) -> T {
    Ambient::current().with(Fork(context)).run(work)
}
