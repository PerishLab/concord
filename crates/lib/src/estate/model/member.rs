use super::Task;
use keel::atom::{int, string};
use keel::resource;

#[resource]
pub(super) struct Member {
    #[field(string, unique = task)]
    name: string,
    #[field(string)]
    source: string,
    #[field(string, opt)]
    branch: string,
    #[relation(Task, many2one, root)]
    task: Task,
}

#[resource]
pub(super) struct Addition {
    #[field(int, unique = member, min = 1)]
    rank: int,
    #[field(string)]
    name: string,
    #[field(string)]
    body: string,
    #[relation(Member, many2one, root)]
    member: Member,
}

#[resource]
pub(super) struct Claim {
    #[field(string, unique = member)]
    path: string,
    #[relation(Member, many2one, root)]
    member: Member,
}

#[resource(frozen)]
pub(super) struct Boundary {
    #[field(string)]
    schema: string,
    #[field(string)]
    plumb: string,
    #[field(string)]
    base: string,
    #[field(string)]
    head: string,
    #[field(string)]
    claim: string,
    #[relation(Member, one2one, root)]
    member: Member,
}
