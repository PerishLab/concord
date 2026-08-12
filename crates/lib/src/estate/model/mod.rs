use keel::atom::{int, string};
use keel::{Graph, resource};

mod current;
mod dependency;
mod domain;
mod member;
mod phase;
mod repository;
mod reservation;

#[resource]
pub(super) struct Space {
    #[field(string, unique, values = ("space",))]
    name: string,
    #[field(int, min = 0)]
    revision: int,
}

#[resource]
pub(super) struct Domain {
    #[field(string, unique = space)]
    name: string,
    #[field(int, min = 0)]
    revision: int,
    #[relation(Space, many2one, root)]
    space: Space,
}

#[resource]
pub(super) struct Task {
    #[field(string, unique = domain)]
    name: string,
    #[field(string, values = ("active", "retired"))]
    state: string,
    #[field(int, min = 0)]
    revision: int,
    #[relation(Domain, many2one, root)]
    domain: Domain,
    #[relation(Task, many2many, closure, weight = string, origin = string)]
    depends: Task,
}

pub(super) fn graph() -> Graph {
    let mut graph = Graph::new();
    graph
        .plug::<Space>()
        .plug::<Domain>()
        .plug::<Task>()
        .plug::<reservation::Reservation>()
        .plug::<domain::Addition>()
        .plug::<repository::Repository>()
        .plug::<repository::Addition>()
        .plug::<member::Member>()
        .plug::<member::Addition>()
        .plug::<member::Claim>()
        .plug::<member::Boundary>()
        .plug::<current::Goal>()
        .plug::<current::Constraint>()
        .plug::<current::Decision>()
        .plug::<current::Focus>()
        .plug::<current::Question>()
        .plug::<current::Next>()
        .plug::<current::Addition>()
        .plug::<phase::Phase>()
        .plug::<phase::Outcome>()
        .plug::<phase::Decision>()
        .plug::<phase::Evidence>()
        .plug::<phase::Carry>()
        .plug::<phase::Addition>()
        .plug::<dependency::Retirement>();
    graph
}
