use super::Domain;
use keel::atom::{int, string};
use keel::resource;

#[resource]
pub(super) struct Repository {
    #[field(string, unique = domain)]
    name: string,
    #[field(string, opt)]
    note: string,
    #[relation(Domain, many2one, root)]
    domain: Domain,
}

#[resource]
pub(super) struct Addition {
    #[field(int, unique = repository, min = 1)]
    rank: int,
    #[field(string)]
    name: string,
    #[field(string)]
    body: string,
    #[relation(Repository, many2one, root)]
    repository: Repository,
}
