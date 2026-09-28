use keel::atom::{int, string};
use keel::resource;

#[resource]
pub(super) struct Anchor {
    #[field(string, unique)]
    node: string,
    #[field(string)]
    owner: string,
    #[field(string)]
    repository: string,
    #[field(int, min = 1)]
    number: int,
    #[field(int, min = 0)]
    revision: int,
}
