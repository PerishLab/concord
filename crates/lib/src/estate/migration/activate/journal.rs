use crate::path::at;
use crate::{Error, Result};
use std::path::{Path, PathBuf};

pub(super) struct Journal {
    shifts: Vec<(PathBuf, PathBuf)>,
    markers: Vec<PathBuf>,
    removed: Vec<PathBuf>,
}

impl Journal {
    pub fn new() -> Self {
        Self {
            shifts: Vec::new(),
            markers: Vec::new(),
            removed: Vec::new(),
        }
    }

    pub fn shift(&mut self, from: &Path, to: &Path) -> Result<()> {
        if !from.exists() || to.exists() {
            return Err(Error::typed(
                "concord.migration.path",
                format!(
                    "migration move requires present source and absent target: {} -> {}",
                    from.display(),
                    to.display()
                ),
            ));
        }
        let parent = to
            .parent()
            .ok_or_else(|| Error::new("migration target has no parent"))?;
        at(parent).directory()?;
        std::fs::rename(from, to)?;
        self.shifts.push((from.to_path_buf(), to.to_path_buf()));
        Ok(())
    }

    pub fn marker(&mut self, path: &Path, content: &str) -> Result<()> {
        if path.exists() {
            return Err(Error::typed(
                "concord.migration.marker",
                format!("refusal marker target is occupied: {}", path.display()),
            ));
        }
        at(path).file(content)?;
        self.markers.push(path.to_path_buf());
        Ok(())
    }

    pub fn prune(&mut self, path: &Path) -> Result<()> {
        if !path.is_dir() || std::fs::read_dir(path)?.next().is_some() {
            return Ok(());
        }
        std::fs::remove_dir(path)?;
        self.removed.push(path.to_path_buf());
        Ok(())
    }

    pub fn rollback(&self) -> Result<()> {
        let mut faults = Vec::new();
        for marker in self.markers.iter().rev() {
            if marker.exists()
                && let Err(error) = std::fs::remove_file(marker)
            {
                faults.push(error.to_string());
            }
        }
        for path in self.removed.iter().rev() {
            if let Err(error) = at(path).directory() {
                faults.push(error.to_string());
            }
        }
        for (from, to) in self.shifts.iter().rev() {
            if to.exists()
                && let Err(error) = std::fs::rename(to, from)
            {
                faults.push(error.to_string());
            }
        }
        if faults.is_empty() {
            return Ok(());
        }
        Err(Error::typed(
            "concord.migration.rollback",
            faults.join("; "),
        ))
    }
}
