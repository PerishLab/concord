use super::Domain;
use keel::atom::string;
use keel::resource;

#[resource]
pub(super) struct Reservation {
    #[field(string, unique = domain)]
    name: string,
    #[relation(Domain, many2one, root)]
    domain: Domain,
}
