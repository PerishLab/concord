use super::Task;
use keel::atom::{int, string};
use keel::resource;

#[resource(frozen)]
pub(super) struct Phase {
    #[field(serial, scope = task)]
    number: int,
    #[relation(Task, many2one, root)]
    task: Task,
}

#[resource(frozen)]
pub(super) struct Outcome {
    #[field(string)]
    body: string,
    #[relation(Phase, one2one, root)]
    phase: Phase,
}

#[resource(frozen)]
pub(super) struct Decision {
    #[field(int, unique = phase, min = 1)]
    rank: int,
    #[field(string)]
    body: string,
    #[relation(Phase, many2one, root)]
    phase: Phase,
}

#[resource(frozen)]
pub(super) struct Evidence {
    #[field(int, unique = phase, min = 1)]
    rank: int,
    #[field(string)]
    body: string,
    #[relation(Phase, many2one, root)]
    phase: Phase,
}

#[resource(frozen)]
pub(super) struct Carry {
    #[field(int, unique = phase, min = 1)]
    rank: int,
    #[field(string)]
    body: string,
    #[relation(Phase, many2one, root)]
    phase: Phase,
}

#[resource(frozen)]
pub(super) struct Addition {
    #[field(int, unique = phase, min = 1)]
    rank: int,
    #[field(string, opt)]
    title: string,
    #[field(string)]
    body: string,
    #[field(string)]
    origin: string,
    #[relation(Phase, many2one, root)]
    phase: Phase,
}
