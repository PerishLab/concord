mod parse;
mod text;

use crate::{Error, Result};
use parse::document;
use std::ops::Range;
use text::{limits, marker, strict};

pub(crate) const RAW: usize = 4 * 1024 * 1024;

struct Shape {
    marker: &'static str,
    family: &'static str,
    bytes: usize,
    lines: usize,
    headings: &'static [(&'static str, &'static str)],
}

const MAIN: Shape = Shape {
    marker: "<!-- concord-memory:v1 -->",
    family: "concord-memory:",
    bytes: 64 * 1024,
    lines: 400,
    headings: &[
        ("goal", "Goal"),
        ("constraints", "Active constraints"),
        ("decisions", "Decisions in force"),
        ("focus", "Current focus"),
        ("questions", "Open questions"),
        ("next", "Next step"),
    ],
};

const PHASE: Shape = Shape {
    marker: "<!-- concord-phase:v1 -->",
    family: "concord-phase:",
    bytes: 128 * 1024,
    lines: 800,
    headings: &[
        ("outcome", "Outcome"),
        ("decisions", "Decisions"),
        ("evidence", "Evidence"),
        ("carry-forward", "Carry-forward"),
    ],
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Kind {
    Legacy,
    V1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Schema {
    Main,
    Phase,
}

pub(crate) struct Extraction<'a> {
    pub sections: Vec<(&'static str, &'a str)>,
    pub preamble: Option<&'a str>,
}

#[derive(Clone, Debug)]
pub(crate) struct Document {
    sections: Vec<Section>,
}

#[derive(Clone, Debug)]
struct Section {
    key: &'static str,
    range: Range<usize>,
}

pub(crate) fn kind(source: &str, schema: Schema) -> Result<Kind> {
    let shape = shape(schema);
    marker(source, shape.marker, shape.family)
}

pub(crate) fn limit(schema: Schema) -> usize {
    shape(schema).bytes
}

pub(crate) fn main(main: &str) -> Result<Option<Document>> {
    limits(main, "MAIN.md", MAIN.bytes, MAIN.lines)?;
    match kind(main, Schema::Main)? {
        Kind::Legacy => Ok(None),
        Kind::V1 => {
            strict(main, "MAIN.md")?;
            Ok(Some(document(main, MAIN.marker, MAIN.headings)?))
        }
    }
}

pub(crate) fn phase(phase: &str, structured: bool) -> Result<()> {
    limits(phase, "phase", PHASE.bytes, PHASE.lines)?;
    let kind = kind(phase, Schema::Phase)?;
    if structured != (kind == Kind::V1) {
        return Err(Error::typed(
            "memory.phase_format",
            "MAIN.md and every retained phase must use the same memory format",
        ));
    }
    if kind == Kind::V1 {
        strict(phase, "phase")?;
        document(phase, PHASE.marker, PHASE.headings)?;
    }
    Ok(())
}

pub(crate) fn import(source: &str, schema: Schema) -> Result<Extraction<'_>> {
    let shape = shape(schema);
    let (marker, parsed) = match schema {
        Schema::Main => {
            let parsed = main(source)?.ok_or_else(|| {
                Error::typed("memory.unstructured", "structured MAIN import required")
            })?;
            (shape.marker, parsed)
        }
        Schema::Phase => {
            phase(source, true)?;
            (
                shape.marker,
                document(source, shape.marker, shape.headings)?,
            )
        }
    };
    let mut sections = Vec::new();
    for section in &parsed.sections {
        let held = &source[section.range.clone()];
        let (_, body) = held.split_once('\n').ok_or_else(|| {
            Error::typed(
                "memory.schema",
                "fixed memory section heading must end with a newline",
            )
        })?;
        sections.push((section.key, body.trim()));
    }
    let end = parsed
        .sections
        .first()
        .map(|section| section.range.start)
        .unwrap_or(marker.len());
    let held = source[marker.len()..end].trim();
    Ok(Extraction {
        sections,
        preamble: (!held.is_empty()).then_some(held),
    })
}

fn shape(schema: Schema) -> &'static Shape {
    match schema {
        Schema::Main => &MAIN,
        Schema::Phase => &PHASE,
    }
}
