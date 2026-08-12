use super::text::first;
use super::{Document, Section};
use crate::{Error, Result};
use pulldown_cmark::{Event, HeadingLevel, Parser, Tag};

pub(super) fn document(
    source: &str,
    marker: &str,
    expected: &'static [(&'static str, &'static str)],
) -> Result<Document> {
    let headings = headings(source)?;
    if headings.len() != expected.len() {
        return Err(Error::typed(
            "memory.schema",
            format!(
                "{} requires exactly {} fixed H2 sections",
                first(source),
                expected.len()
            ),
        ));
    }
    let mut sections = Vec::new();
    for (index, ((start, title), (key, wanted))) in headings.iter().zip(expected.iter()).enumerate()
    {
        if title != wanted {
            return Err(Error::typed(
                "memory.schema",
                format!(
                    "section {} must be `## {wanted}`, found `## {title}`",
                    index + 1
                ),
            ));
        }
        let end = headings
            .get(index + 1)
            .map_or(source.len(), |(start, _)| *start);
        sections.push(Section {
            key,
            range: *start..end,
        });
    }
    if marker.len() + 1 > sections[0].range.start {
        return Err(Error::typed(
            "memory.schema",
            "memory marker must precede every fixed H2 section",
        ));
    }
    Ok(Document { sections })
}

pub(super) fn headings(markdown: &str) -> Result<Vec<(usize, String)>> {
    let mut nesting = 0usize;
    let mut found = Vec::new();
    for (event, range) in Parser::new(markdown).into_offset_iter() {
        match event {
            Event::Start(Tag::Heading {
                level: HeadingLevel::H2,
                ..
            }) if nesting == 0 => {
                found.push((range.start, title(markdown, range.start)?));
                nesting += 1;
            }
            Event::Start(_) => nesting += 1,
            Event::End(_) => nesting = nesting.saturating_sub(1),
            _ => {}
        }
    }
    Ok(found)
}

fn title(markdown: &str, start: usize) -> Result<String> {
    let end = markdown[start..]
        .find('\n')
        .map_or(markdown.len(), |offset| start + offset);
    markdown[start..end]
        .strip_prefix("## ")
        .map(str::to_string)
        .ok_or_else(|| {
            Error::typed(
                "memory.schema",
                "fixed top-level sections must use unindented `## Title` headings",
            )
        })
}
