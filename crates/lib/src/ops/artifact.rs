use super::{Plan, action};
use crate::model::component;
use crate::{Error, Result, Space};

impl Space {
    pub fn memory_remove(&self, identity: &str, apply: bool) -> Result<Plan> {
        let task = self.resolve(identity)?;
        task.ensure_exact()?;
        let root = crate::Memory::new(&task).root();
        if !root.is_dir() {
            return Err(Error::new("task memory does not exist"));
        }
        let actions = vec![action("remove-tree", &root, "exact task memory target")];
        if apply {
            let _lock = self.lock()?;
            let task = self.resolve(identity)?;
            task.ensure_exact()?;
            std::fs::remove_dir_all(crate::Memory::new(&task).root())?;
        }
        Ok(Plan::new("memory.remove", actions, apply))
    }

    pub fn resource_remove(&self, identity: &str, name: &str, apply: bool) -> Result<Plan> {
        component("resource seat", name)?;
        let task = self.resolve(identity)?;
        task.ensure_exact()?;
        let path = crate::Memory::new(&task)
            .root()
            .join("resources")
            .join(name);
        if !path.is_dir() {
            return Err(Error::new(format!(
                "resource seat does not exist: {}",
                path.display()
            )));
        }
        let actions = vec![action("remove-tree", &path, "exact resource seat target")];
        if apply {
            let _lock = self.lock()?;
            let task = self.resolve(identity)?;
            task.ensure_exact()?;
            std::fs::remove_dir_all(
                crate::Memory::new(&task)
                    .root()
                    .join("resources")
                    .join(name),
            )?;
        }
        Ok(Plan::new("resource.remove", actions, apply))
    }
}
