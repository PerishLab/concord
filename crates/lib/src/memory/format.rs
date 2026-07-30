mod parse;
mod text;

use crate::{Error, Result};
use parse::{document, headings, subset_sections};
use std::ops::Range;
use text::{first_line, limits, marker_kind, strict_text};

pub const MAIN_MARKER: &str = "<!-- concord-memory:v1 -->";
pub const PHASE_MARKER: &str = "<!-- concord-phase:v1 -->";
pub const PATCH_MARKER_PREFIX: &str = "<!-- concord-memory-patch:v1 revision=";
pub const MAX_MAIN_BYTES: usize = 64 * 1024;
pub const MAX_MAIN_LINES: usize = 400;
pub const MAX_PHASE_BYTES: usize = 128 * 1024;
pub const MAX_PHASE_LINES: usize = 800;
pub const MAX_RAW_READ_BYTES: usize = 4 * 1024 * 1024;

const MAIN_HEADINGS: [(&str, &str); 6] = [
    ("goal", "Goal"),
    ("constraints", "Active constraints"),
    ("decisions", "Decisions in force"),
    ("focus", "Current focus"),
    ("questions", "Open questions"),
    ("next", "Next step"),
];
const PHASE_HEADINGS: [(&str, &str); 4] = [
    ("outcome", "Outcome"),
    ("decisions", "Decisions"),
    ("evidence", "Evidence"),
    ("carry-forward", "Carry-forward"),
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Kind {
    Legacy,
    V1,
}

#[derive(Clone, Debug)]
pub struct Document {
    sections: Vec<Section>,
}

#[derive(Clone, Debug)]
struct Section {
    key: &'static str,
    range: Range<usize>,
}

#[derive(Clone, Debug)]
pub struct Patch {
    pub revision: String,
    sections: Vec<Section>,
}

pub fn main_kind(source: &str) -> Result<Kind> {
    marker_kind(source, MAIN_MARKER, "concord-memory:")
}

pub fn phase_kind(source: &str) -> Result<Kind> {
    marker_kind(source, PHASE_MARKER, "concord-phase:")
}

pub fn validate_main(main: &str) -> Result<Option<Document>> {
    limits(main, "MAIN.md", MAX_MAIN_BYTES, MAX_MAIN_LINES)?;
    match main_kind(main)? {
        Kind::Legacy => Ok(None),
        Kind::V1 => {
            strict_text(main, "MAIN.md")?;
            Ok(Some(document(main, MAIN_MARKER, &MAIN_HEADINGS)?))
        }
    }
}

pub fn validate_phase(phase: &str, structured: bool) -> Result<()> {
    limits(phase, "phase", MAX_PHASE_BYTES, MAX_PHASE_LINES)?;
    let kind = phase_kind(phase)?;
    if structured != (kind == Kind::V1) {
        return Err(Error::typed(
            "memory.phase_format",
            "MAIN.md and every retained phase must use the same memory format",
        ));
    }
    if kind == Kind::V1 {
        strict_text(phase, "phase")?;
        document(phase, PHASE_MARKER, &PHASE_HEADINGS)?;
    }
    Ok(())
}

pub fn project(current: &str, revision: &str, keys: &[String]) -> Result<String> {
    let parsed = validate_main(current)?.ok_or_else(|| {
        Error::typed(
            "memory.unstructured",
            "section projection requires concord-memory:v1",
        )
    })?;
    let selected = selected(&parsed, keys)?;
    let mut projected = format!("{PATCH_MARKER_PREFIX}{revision} -->\n\n");
    for section in selected {
        projected.push_str(&current[section.range.clone()]);
    }
    Ok(projected)
}

pub fn parse_patch(patch_source: &str) -> Result<Patch> {
    limits(patch_source, "memory patch", MAX_MAIN_BYTES, MAX_MAIN_LINES)?;
    strict_text(patch_source, "memory patch")?;
    let first = first_line(patch_source);
    let revision = first
        .strip_prefix(PATCH_MARKER_PREFIX)
        .and_then(|value| value.strip_suffix(" -->"))
        .ok_or_else(|| {
            Error::typed(
                "memory.patch_marker",
                "memory patch must start with a concord-memory-patch:v1 revision marker",
            )
        })?;
    if revision.len() != 64
        || !revision
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(Error::typed(
            "memory.patch_revision",
            "memory patch revision must be a lowercase SHA-256 value",
        ));
    }
    let headings = headings(patch_source)?;
    if headings.is_empty() {
        return Err(Error::typed(
            "memory.patch_empty",
            "memory patch must contain at least one section",
        ));
    }
    let sections = subset_sections(patch_source, headings, &MAIN_HEADINGS)?;
    let marker_end = first.len() + 1;
    if !patch_source[marker_end..sections[0].range.start]
        .trim()
        .is_empty()
    {
        return Err(Error::typed(
            "memory.patch_preamble",
            "memory patch cannot contain content outside its selected sections",
        ));
    }
    Ok(Patch {
        revision: revision.to_string(),
        sections,
    })
}

pub fn apply_patch(current: &str, patch_content: &str, patch: &Patch) -> Result<String> {
    let parsed = validate_main(current)?.ok_or_else(|| {
        Error::typed(
            "memory.unstructured",
            "memory patch requires concord-memory:v1",
        )
    })?;
    let mut edits = Vec::new();
    for replacement in &patch.sections {
        let held = parsed
            .sections
            .iter()
            .find(|section| section.key == replacement.key)
            .expect("validated MAIN has every fixed section");
        edits.push((
            held.range.clone(),
            patch_content[replacement.range.clone()].to_string(),
        ));
    }
    edits.sort_by_key(|edit| std::cmp::Reverse(edit.0.start));
    let mut updated = current.to_string();
    for (range, replacement) in edits {
        updated.replace_range(range, &replacement);
    }
    validate_main(&updated)?;
    Ok(updated)
}

fn selected<'a>(document: &'a Document, keys: &[String]) -> Result<Vec<&'a Section>> {
    if keys.is_empty() {
        return Err(Error::typed(
            "memory.section_empty",
            "at least one --section is required for a projection",
        ));
    }
    let mut indexes = Vec::new();
    for key in keys {
        let index = MAIN_HEADINGS
            .iter()
            .position(|(candidate, _)| candidate == key)
            .ok_or_else(|| {
                Error::typed(
                    "memory.section_unknown",
                    format!("unknown MAIN.md section key `{key}`"),
                )
            })?;
        if indexes.contains(&index) {
            return Err(Error::typed(
                "memory.section_duplicate",
                format!("duplicate MAIN.md section key `{key}`"),
            ));
        }
        indexes.push(index);
    }
    indexes.sort_unstable();
    Ok(indexes
        .into_iter()
        .map(|index| &document.sections[index])
        .collect())
}
