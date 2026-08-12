use super::super::task::reserved;
use super::super::{Estate, World};
use crate::model::component;
use crate::{Error, Result};

pub(super) enum Target {
    Held(i64),
    Fresh((i64, String)),
}

pub(super) async fn fresh(estate: &Estate, world: &World, identity: &str) -> Result<(i64, String)> {
    let (domain, name) = identity.split_once('/').ok_or_else(|| {
        Error::typed(
            "concord.task.identity",
            "--create-target requires an exact domain/task identity",
        )
    })?;
    component("domain name", domain)?;
    component("task name", name)?;
    let root = world.domain(domain).ok_or_else(|| {
        Error::typed(
            "concord.domain.absent",
            format!("unknown managed domain {domain}"),
        )
    })?;
    if reserved(estate, root, name).await? {
        return Err(Error::typed(
            "concord.task.reserved",
            format!("Task name is already reserved: {identity}"),
        ));
    }
    let path = estate.space.join(domain).join(".tasks").join(name);
    if path.exists() || std::fs::symlink_metadata(&path).is_ok() {
        return Err(Error::typed(
            "concord.task.territory",
            format!(
                "Task seat is occupied by foreign territory: {}",
                path.display()
            ),
        ));
    }
    Ok((root, name.to_string()))
}
