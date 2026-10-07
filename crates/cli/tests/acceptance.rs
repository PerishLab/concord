#[cfg(unix)]
mod acceptance {
    mod apply;
    mod closing;
    mod conditions;
    mod evaluate;
    mod observe;
    mod prepare;
    mod readiness;
    mod recovery;
    mod release;
    mod world;
}

#[cfg(unix)]
#[path = "seat/spawn.rs"]
mod spawn;
