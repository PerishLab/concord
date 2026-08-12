use super::Domain;
use keel::atom::{int, string};
use keel::resource;

#[resource]
pub(super) struct Addition {
    #[field(int, unique = domain, min = 1)]
    rank: int,
    #[field(string)]
    name: string,
    #[field(string)]
    body: string,
    #[relation(Domain, many2one, root)]
    domain: Domain,
}
