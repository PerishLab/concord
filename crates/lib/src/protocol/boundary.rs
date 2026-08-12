use crate::{Boundary, claim};

pub const PLUMB: &str = env!("CONCORD_PLUMB_VERSION");

pub(crate) fn valid(proof: &Boundary, write: &[String], head: &str) -> bool {
    if proof.schema != plumb::boundary::SCHEMA || proof.plumb != PLUMB {
        return false;
    }
    proof.head == head && proof.claim == claim::digest(write)
}
