use super::Task;
use super::member::Member;
use keel::atom::{int, string};
use keel::resource;

#[resource]
pub(super) struct Issue {
    #[field(string)]
    provider: string,
    #[field(string)]
    owner: string,
    #[field(string)]
    repository: string,
    #[field(int, min = 1)]
    number: int,
    #[relation(Task, one2one, root)]
    task: Task,
}

#[resource]
pub(super) struct Change {
    #[field(string)]
    provider: string,
    #[field(string)]
    owner: string,
    #[field(string)]
    repository: string,
    #[field(int, min = 1)]
    number: int,
    #[relation(Member, one2one, root)]
    member: Member,
}
