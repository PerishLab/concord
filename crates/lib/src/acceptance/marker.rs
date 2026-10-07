use pulldown_cmark::{Event, Parser, Tag, TagEnd};

use super::Marker;
use crate::Result;

const PREFIX: &str = "<!-- concord.acceptance/";
const VERSION: &str = "<!-- concord.acceptance/v1\n";
const LIMIT: usize = 65_536;

impl Marker {
    pub fn render(&self) -> Result<String> {
        super::validation::validate(self)?;
        let json = serde_json::to_string(self).map_err(|error| super::fault(error.to_string()))?;
        let marker = format!("{VERSION}{json}\n-->");
        bounded(&marker)?;
        Ok(marker)
    }

    pub fn parse(body: &str) -> Result<Option<Self>> {
        bounded(body)?;
        let mut found = None;
        let mut quotes = 0;
        let mut block = String::new();
        for event in Parser::new(body) {
            match event {
                Event::Start(Tag::BlockQuote(_)) => quotes += 1,
                Event::End(TagEnd::BlockQuote(_)) => quotes -= 1,
                Event::Start(Tag::HtmlBlock) => block.clear(),
                Event::Html(html) if quotes == 0 => block.push_str(&html),
                Event::End(TagEnd::HtmlBlock) if quotes == 0 => {
                    inspect(&block, &mut found)?;
                    block.clear();
                }
                Event::InlineHtml(html) if quotes == 0 => {
                    inspect(&html, &mut found)?;
                }
                _ => {}
            }
        }
        Ok(found)
    }
}

fn inspect(html: &str, found: &mut Option<Marker>) -> Result<()> {
    let Some(start) = html.find(PREFIX) else {
        return Ok(());
    };
    let tail = &html[start..];
    let Some(content) = tail.strip_prefix(VERSION) else {
        return Err(super::fault(
            "unsupported or malformed acceptance marker version",
        ));
    };
    let Some(end) = content.find("-->") else {
        return Err(super::fault("unterminated acceptance marker"));
    };
    if found.is_some() || content[end + 3..].contains(PREFIX) {
        return Err(super::fault(
            "multiple acceptance markers in one provider comment",
        ));
    }
    let marker: Marker = serde_json::from_str(content[..end].trim())
        .map_err(|error| super::fault(format!("malformed acceptance marker: {error}")))?;
    super::validation::validate(&marker)?;
    *found = Some(marker);
    Ok(())
}

fn bounded(body: &str) -> Result<()> {
    if body.len() <= LIMIT {
        return Ok(());
    }
    Err(super::fault(
        "acceptance comment exceeds the provider body bound",
    ))
}
