mod execution;
mod reference;
use keel::Graph;

pub(super) fn graph() -> Graph {
    let mut graph = Graph::new();
    graph
        .plug::<reference::Anchor>()
        .plug::<execution::Integration>()
        .plug::<execution::Member>()
        .plug::<execution::Claim>()
        .plug::<execution::Boundary>()
        .plug::<execution::Change>();
    graph
}
