use super::{Memory, Read, format, read};
use crate::{Error, Result};
use std::path::Path;

pub(crate) struct Epoch {
    pub number: u32,
    pub path: String,
}

impl Memory<'_> {
    pub(crate) fn phases(&self) -> Result<Vec<Epoch>> {
        let main = self.read()?;
        let structured = format::kind(&main.content, format::Schema::Main)? == format::Kind::V1;
        entries(&self.root().join("phases"), structured)
    }

    pub(crate) fn phase(&self, number: u32) -> Result<Read> {
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
}

fn entries(root: &Path, structured: bool) -> Result<Vec<Epoch>> {
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
        format::phase(&read.content, structured)?;
        phases.push(Epoch {
            number,
            path: read.path,
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
