use pulldown_cmark::{Event, HeadingLevel, Options, Parser, Tag, TagEnd};
use std::collections::BTreeMap;

pub fn sections(body: &str) -> BTreeMap<String, String> {
    let mut result = BTreeMap::new();
    let mut current: Option<(String, usize)> = None;
    let mut heading = None;
    let mut quotes = 0;
    for (event, range) in Parser::new(body).into_offset_iter() {
        match event {
            Event::Start(Tag::BlockQuote(_)) => quotes += 1,
            Event::End(TagEnd::BlockQuote(_)) => quotes -= 1,
            Event::Start(Tag::Heading {
                level: HeadingLevel::H2,
                ..
            }) if quotes == 0 => {
                flush(&mut result, &mut current, body, range.start);
                heading = Some(String::new());
            }
            Event::Text(text) | Event::Code(text) if quotes == 0 => {
                append(&mut heading, &text);
            }
            Event::End(TagEnd::Heading(HeadingLevel::H2)) if quotes == 0 => {
                current = heading
                    .take()
                    .map(|name| (name.trim().to_ascii_lowercase(), range.end));
            }
            _ => {}
        }
    }
    flush(&mut result, &mut current, body, body.len());
    result
}

fn append(heading: &mut Option<String>, text: &str) {
    if let Some(name) = heading {
        name.push_str(text);
    }
}

fn flush(
    result: &mut BTreeMap<String, String>,
    current: &mut Option<(String, usize)>,
    body: &str,
    end: usize,
) {
    if let Some((name, start)) = current.take() {
        insert(result, name, &body[start..end]);
    }
}

fn insert(result: &mut BTreeMap<String, String>, name: String, text: &str) {
    use std::collections::btree_map::Entry;
    match result.entry(name) {
        Entry::Vacant(entry) => {
            entry.insert(text.trim().into());
        }
        Entry::Occupied(mut entry) => entry.get_mut().clear(),
    }
}

pub fn checkboxes(section: &str) -> (usize, usize) {
    let mut total = 0;
    let mut open = 0;
    let mut quotes = 0;
    for event in Parser::new_ext(section, Options::ENABLE_TASKLISTS) {
        match event {
            Event::Start(Tag::BlockQuote(_)) => quotes += 1,
            Event::End(TagEnd::BlockQuote(_)) => quotes -= 1,
            Event::TaskListMarker(checked) if quotes == 0 => {
                total += 1;
                open += usize::from(!checked);
            }
            _ => {}
        }
    }
    (total, open)
}
