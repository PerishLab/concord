use super::path::{Trace, foreign};
use crate::memory::Memory;
use crate::model::component;
use crate::{Error, Result};
use std::path::Path;

pub(super) fn read(
    trace: &mut Trace<'_>,
    task: &crate::Legacy,
) -> Result<(Option<String>, Vec<String>, Vec<String>)> {
    let memory = Memory::new(task);
    let root = memory.root();
    if !root.exists() {
        return Ok((None, Vec::new(), Vec::new()));
    }
    territory(&root)?;
    trace.one(&root, "directory")?;
    let main = memory.read()?;
    trace.one(Path::new(&main.path), "main")?;
    let mut phases = Vec::new();
    let lineage = root.join("phases");
    if lineage.exists() {
        trace.one(&lineage, "directory")?;
    }
    for entry in memory.phases()? {
        let phase = memory.phase(entry.number)?;
        trace.one(Path::new(&phase.path), "phase")?;
        phases.push(phase.content);
    }
    let resources = root.join("resources");
    let mut artifacts = Vec::new();
    if resources.exists() {
        if !resources.is_dir() {
            return Err(foreign(&resources));
        }
        trace.one(&resources, "directory")?;
        for entry in std::fs::read_dir(&resources)? {
            let entry = entry?;
            let name = entry
                .file_name()
                .into_string()
                .map_err(|_| foreign(&entry.path()))?;
            component("artifact name", &name)?;
            if !entry.file_type()?.is_dir() {
                return Err(foreign(&entry.path()));
            }
            trace.walk(&entry.path(), "artifact", false)?;
            artifacts.push(name);
        }
        artifacts.sort();
    }
    Ok((Some(main.content), phases, artifacts))
}

fn territory(root: &Path) -> Result<()> {
    for entry in std::fs::read_dir(root)? {
        let entry = entry?;
        if !matches!(
            entry.file_name().to_str(),
            Some("MAIN.md" | "phases" | "resources")
        ) {
            return Err(Error::typed(
                "concord.migration.territory",
                format!("unknown memory territory: {}", entry.path().display()),
            ));
        }
    }
    Ok(())
}
