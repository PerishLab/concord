use super::reference::Anchor;
use keel::atom::{int, string};
use keel::resource;

#[resource]
pub(super) struct Integration {
    #[field(string, unique)]
    node: string,
    #[field(string)]
    owner: string,
    #[field(string, unique = owner)]
    repository: string,
    #[field(string, unique)]
    path: string,
    #[field(string, unique)]
    common: string,
    #[field(string)]
    remote: string,
    #[field(string)]
    branch: string,
}

#[resource]
pub(super) struct IssueMember {
    #[field(string)]
    branch: string,
    #[relation(Anchor, one2one, root)]
    anchor: Anchor,
    #[relation(Integration, many2one)]
    integration: Integration,
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
