use super::{Memory, MemoryChange, MemoryRead, format, read, revision_error};
use crate::path::at;
use crate::{Error, Result};
use serde::Serialize;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Serialize)]
pub struct PhaseEntry {
    pub number: u32,
    pub path: String,
    pub revision: String,
    pub structured: bool,
}

impl Memory<'_> {
    pub fn settle(
        &self,
        expected: &str,
        phase: &str,
        current: &str,
    ) -> Result<(MemoryChange, PathBuf)> {
        format::validate_main(current)?;
        let _lock = self.task.lock()?;
        self.task.ensure_exact()?;
        let held = self.read()?;
        if held.revision != expected {
            return Err(revision_error(expected, &held.revision));
        }
        if held.content == current {
            return Err(Error::typed(
                "memory.settle_unchanged",
                "memory settle requires MAIN.md to change before allocating a phase",
            ));
        }
        let held_kind = format::main_kind(&held.content)?;
        let current_kind = format::main_kind(current)?;
        if held_kind == format::Kind::V1 && current_kind != format::Kind::V1 {
            return Err(Error::typed(
                "memory.downgrade",
                "ordinary memory settle cannot downgrade concord-memory:v1",
            ));
        }
        let phase_kind = format::phase_kind(phase)?;
        if (current_kind == format::Kind::V1) != (phase_kind == format::Kind::V1) {
            return Err(Error::typed(
                "memory.settle_format",
                "memory settle requires MAIN.md and PHASE to use the same format",
            ));
        }
        format::validate_phase(phase, current_kind == format::Kind::V1)?;
        let phases = self.root().join("phases");
        let existing = entries(&phases, current_kind == format::Kind::V1)?;
        at(&phases).directory()?;
        let path = phases.join(name(existing.len() as u32));
        at(&path).file(phase)?;
        match self.write_held(expected, current) {
            Ok(read) => Ok((read, path)),
            Err(error) => {
                let current_revision = self
                    .read()
                    .map(|read| read.revision)
                    .unwrap_or_else(|_| held.revision);
                Err(Error::typed(
                    "memory.settle_partial",
                    format!("{error}; complete phase remains at {}", path.display()),
                )
                .with_details(serde_json::json!({
                    "applied": true,
                    "phase": path.display().to_string(),
                    "current_revision": current_revision,
                })))
            }
        }
    }

    pub fn phases(&self) -> Result<Vec<PhaseEntry>> {
        let main = self.read()?;
        let structured = format::main_kind(&main.content)? == format::Kind::V1;
        entries(&self.root().join("phases"), structured)
    }

    pub fn phase(&self, number: u32) -> Result<MemoryRead> {
        let phases = self.phases()?;
        let entry = phases
            .iter()
            .find(|entry| entry.number == number)
            .ok_or_else(|| {
                Error::typed(
                    "memory.phase_missing",
                    format!("memory phase {number} does not exist"),
                )
            })?;
        read(Path::new(&entry.path))
    }

    pub(super) fn ensure_structured_transition(&self) -> Result<()> {
        entries(&self.root().join("phases"), true).map(|_| ())
    }
}

fn entries(root: &Path, structured: bool) -> Result<Vec<PhaseEntry>> {
    if !root.exists() {
        return Ok(Vec::new());
    }
    if !root.is_dir() {
        return Err(Error::typed(
            "memory.phase_directory",
            format!("memory phases path is not a directory: {}", root.display()),
        ));
    }
    let mut held = Vec::new();
    for entry in std::fs::read_dir(root)? {
        let entry = entry?;
        let path = entry.path();
        if !entry.file_type()?.is_file() {
            return Err(Error::typed(
                "memory.phase_entry",
                format!(
                    "memory phases contains a non-file entry: {}",
                    path.display()
                ),
            ));
        }
        let filename = entry.file_name();
        let filename = filename.to_str().ok_or_else(|| {
            Error::typed(
                "memory.phase_name",
                format!("memory phase name is not UTF-8: {}", path.display()),
            )
        })?;
        let number = number(filename).ok_or_else(|| {
            Error::typed(
                "memory.phase_name",
                format!("malformed memory phase name: {}", path.display()),
            )
        })?;
        held.push((number, path));
    }
    held.sort_by_key(|(number, _)| *number);
    let mut phases = Vec::new();
    for (expected, (number, path)) in held.into_iter().enumerate() {
        if number != expected as u32 {
            return Err(Error::typed(
                "memory.phase_gap",
                format!(
                    "memory phases must be contiguous from 0; expected {expected}, found {number}"
                ),
            ));
        }
        let read = read(&path)?;
        format::validate_phase(&read.content, structured)?;
        phases.push(PhaseEntry {
            number,
            path: read.path,
            revision: read.revision,
            structured: format::phase_kind(&read.content)? == format::Kind::V1,
        });
    }
    Ok(phases)
}

fn number(filename: &str) -> Option<u32> {
    let digits = filename.strip_prefix("PHASE-")?.strip_suffix(".md")?;
    if digits.len() < 2 || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    let number = digits.parse::<u32>().ok()?;
    (name(number) == filename).then_some(number)
}

fn name(number: u32) -> String {
    format!("PHASE-{number:02}.md")
}
