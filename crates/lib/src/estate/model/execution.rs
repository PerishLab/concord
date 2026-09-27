use super::reference::Anchor;
use keel::atom::{int, string};
use keel::resource;

#[resource]
pub(super) struct IssueMember {
    #[field(string, unique = anchor)]
    name: string,
    #[field(string)]
    source: string,
    #[field(string)]
    branch: string,
    #[relation(Anchor, many2one, root)]
    anchor: Anchor,
}

#[resource]
pub(super) struct IssueClaim {
    #[field(string, unique = member)]
    path: string,
    #[relation(IssueMember, many2one, root)]
    member: IssueMember,
    #[relation(Anchor, many2one)]
    anchor: Anchor,
}

#[resource(frozen)]
pub(super) struct IssueBoundary {
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
    #[relation(IssueMember, one2one, root)]
    member: IssueMember,
    #[relation(Anchor, many2one)]
    anchor: Anchor,
}

#[resource]
pub(super) struct IssueChange {
    #[field(string)]
    provider: string,
    #[field(string)]
    owner: string,
    #[field(string)]
    repository: string,
    #[field(int, min = 1)]
    number: int,
    #[relation(IssueMember, many2one, root)]
    member: IssueMember,
    #[relation(Anchor, many2one)]
    anchor: Anchor,
}
