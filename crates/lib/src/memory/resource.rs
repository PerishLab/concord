use super::{Memory, copy};
use crate::path::at;
use crate::{Error, ImportPreflight, Result};
use std::path::{Path, PathBuf};

impl Memory<'_> {
    pub fn allocate(&self, name: &str) -> Result<PathBuf> {
        crate::model::component("resource seat", name)?;
        let _lock = self.task.lock()?;
        self.task.ensure_exact()?;
        self.require_root()?;
        let root = self.root().join("resources");
        at(&root).directory()?;
        let seat = root.join(name);
        if seat.exists() {
            return Err(Error::new(format!(
                "resource seat already exists: {}",
                seat.display()
            )));
        }
        at(&seat).directory()?;
        Ok(seat)
    }

    pub fn import(&self, name: &str, source: &Path) -> Result<PathBuf> {
        crate::model::component("resource seat", name)?;
        let _lock = self.task.lock()?;
        self.task.ensure_exact()?;
        self.require_root()?;
        let root = self.root().join("resources");
        let seat = root.join(name);
        if seat.exists() {
            return Err(Error::new(format!(
                "resource seat already exists: {}",
                seat.display()
            )));
        }
        let preflight = crate::audit::resource::import_preflight(source, &seat, &self.root())?;
        let source = PathBuf::from(preflight.source);
        at(&root).directory()?;
        at(&seat).directory()?;
        let result = if source.is_dir() {
            copy::tree(&source, &seat)
        } else {
            let filename = source
                .file_name()
                .ok_or_else(|| Error::new("resource source has no filename"))?;
            copy::file(&source, &seat.join(filename))
        };
        if let Err(error) = result {
            let _ = std::fs::remove_dir_all(&seat);
            return Err(error);
        }
        Ok(seat)
    }

    pub fn preflight_import(&self, name: &str, source: &Path) -> Result<ImportPreflight> {
        crate::model::component("resource seat", name)?;
        self.task.ensure_exact()?;
        self.require_root()?;
        let seat = self.root().join("resources").join(name);
        if seat.exists() {
            return Err(Error::new(format!(
                "resource seat already exists: {}",
                seat.display()
            )));
        }
        crate::audit::resource::import_preflight(source, &seat, &self.root())
    }

    pub fn resources(&self) -> Result<Vec<PathBuf>> {
        let root = self.root().join("resources");
        if !root.exists() {
            return Ok(Vec::new());
        }
        let mut seats = std::fs::read_dir(root)?
            .filter_map(|entry| entry.ok())
            .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_dir()))
            .map(|entry| entry.path())
            .collect::<Vec<_>>();
        seats.sort();
        Ok(seats)
    }

    pub fn resource(&self, name: &str) -> Result<PathBuf> {
        crate::model::component("resource seat", name)?;
        let seat = self.root().join("resources").join(name);
        if !seat.is_dir() {
            return Err(Error::new(format!(
                "resource seat does not exist: {}",
                seat.display()
            )));
        }
        Ok(seat)
    }

    fn require_root(&self) -> Result<()> {
        if self.main().is_file() {
            Ok(())
        } else {
            Err(Error::new("resource seats require initialized task memory"))
        }
    }
}
