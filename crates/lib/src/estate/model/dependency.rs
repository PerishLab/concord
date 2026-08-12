use super::{Space, Task};
use keel::atom::{int, string};
use keel::resource;

#[resource(frozen)]
pub(super) struct Retirement {
    #[field(serial, scope = space)]
    number: int,
    #[field(string)]
    weight: string,
    #[field(string)]
    origin: string,
    #[field(string)]
    reason: string,
    #[relation(Space, many2one, root)]
    space: Space,
    #[relation(Task, many2one)]
    source: Task,
    #[relation(Task, many2one)]
    target: Task,
}
