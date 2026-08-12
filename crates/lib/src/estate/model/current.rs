use super::Task;
use keel::atom::{int, string};
use keel::resource;

#[resource]
pub(super) struct Goal {
    #[field(string)]
    body: string,
    #[relation(Task, one2one, root)]
    task: Task,
}

#[resource]
pub(super) struct Constraint {
    #[field(int, unique = task, min = 1)]
    rank: int,
    #[field(string)]
    body: string,
    #[relation(Task, many2one, root)]
    task: Task,
}

#[resource]
pub(super) struct Decision {
    #[field(int, unique = task, min = 1)]
    rank: int,
    #[field(string)]
    body: string,
    #[relation(Task, many2one, root)]
    task: Task,
}

#[resource]
pub(super) struct Focus {
    #[field(string)]
    body: string,
    #[relation(Task, one2one, root)]
    task: Task,
}

#[resource]
pub(super) struct Question {
    #[field(int, unique = task, min = 1)]
    rank: int,
    #[field(string)]
    body: string,
    #[relation(Task, many2one, root)]
    task: Task,
}

#[resource]
pub(super) struct Next {
    #[field(string)]
    body: string,
    #[relation(Task, one2one, root)]
    task: Task,
}

#[resource]
pub(super) struct Addition {
    #[field(int, unique = task, min = 1)]
    rank: int,
    #[field(string, opt)]
    title: string,
    #[field(string)]
    body: string,
    #[field(string)]
    origin: string,
    #[relation(Task, many2one, root)]
    task: Task,
}
