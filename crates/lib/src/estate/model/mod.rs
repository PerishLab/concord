mod execution;
mod reference;
use keel::Graph;

pub(super) fn graph() -> Graph {
    let mut graph = Graph::new();
    graph
        .plug::<reference::Anchor>()
        .plug::<execution::IssueMember>()
        .plug::<execution::IssueClaim>()
        .plug::<execution::IssueBoundary>()
        .plug::<execution::IssueChange>();
    graph
}
