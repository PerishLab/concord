use keel::atom::{Kind as Atom, string};
use keel::bond::Kind as Bond;
use keel::resource;
use keel::spec::{Resource, Rule, Spec};

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

pub(super) struct Member;

impl Resource for Member {
    fn name() -> &'static str {
        "IssueMember"
    }

    fn spec() -> Spec {
        Spec::build("IssueMember")
            .field("branch", Atom::Text)
            .field("base", Atom::Text)
            .root("anchor", Bond::One2one, "Anchor")
            .bond("integration", Bond::Many2one, "Integration", &[])
            .seal()
    }
}

pub(super) struct Claim;

impl Resource for Claim {
    fn name() -> &'static str {
        "IssueClaim"
    }

    fn spec() -> Spec {
        Spec::build("IssueClaim")
            .per("path", Atom::Text, &["member"])
            .root("member", Bond::Many2one, "IssueMember")
            .bond("anchor", Bond::Many2one, "Anchor", &[])
            .seal()
    }
}

pub(super) struct Boundary;

impl Resource for Boundary {
    fn name() -> &'static str {
        "IssueBoundary"
    }

    fn spec() -> Spec {
        Spec::build("IssueBoundary")
            .field("schema", Atom::Text)
            .field("plumb", Atom::Text)
            .field("base", Atom::Text)
            .field("head", Atom::Text)
            .field("claim", Atom::Text)
            .root("member", Bond::One2one, "IssueMember")
            .bond("anchor", Bond::Many2one, "Anchor", &[])
            .freeze()
            .seal()
    }
}

pub(super) struct Change;

impl Resource for Change {
    fn name() -> &'static str {
        "IssueChange"
    }

    fn spec() -> Spec {
        Spec::build("IssueChange")
            .field("provider", Atom::Text)
            .field("owner", Atom::Text)
            .field("repository", Atom::Text)
            .field("number", Atom::Int)
            .rule("number", Rule::new().min(1))
            .root("member", Bond::Many2one, "IssueMember")
            .bond("anchor", Bond::Many2one, "Anchor", &[])
            .seal()
    }
}
