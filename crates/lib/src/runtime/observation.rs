use locus::{Context, Engine};
use std::cell::RefCell;

type Seat = (&'static Engine, Context);

thread_local! {
    static FORK: RefCell<Option<Context>> = const { RefCell::new(None) };
}

pub(crate) fn view() -> Option<Seat> {
    let (engine, root) = plumb::observation::view()?;
    let context = FORK
        .with(|fork| fork.borrow().clone())
        .unwrap_or_else(|| root.clone());
    Some((engine, context))
}

pub(crate) fn borrow(seat: &Seat) -> (&'static Engine, &Context) {
    (seat.0, &seat.1)
}

pub(crate) fn fork(frame: Option<&Context>) -> Option<Context> {
    let (engine, _) = plumb::observation::view()?;
    engine.fork(frame?).ok()
}

pub(crate) fn within<T>(context: Option<Context>, work: impl FnOnce() -> T) -> T {
    let previous = FORK.with(|fork| fork.replace(context));
    let result = work();
    FORK.with(|fork| fork.replace(previous));
    result
}
