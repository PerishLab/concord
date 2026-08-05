use super::{Memory, format};
use crate::{Error, Result};
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum MemoryBrief {
    Absent,
    Legacy {
        revision: String,
    },
    Structured {
        revision: String,
        sections: BTreeMap<String, TextPreview>,
    },
    Unavailable {
        code: String,
        message: String,
    },
}

#[derive(Clone, Debug, Serialize)]
pub struct TextPreview {
    pub text: String,
    pub bytes: usize,
    pub truncated: bool,
}

impl MemoryBrief {
    pub fn unavailable(error: &Error) -> Self {
        Self::Unavailable {
            code: error.code().to_string(),
            message: error.message().to_string(),
        }
    }
}

impl<'a> Memory<'a> {
    pub fn brief_sections(&self, keys: &[String], max_bytes: usize) -> Result<MemoryBrief> {
        if !self.root().exists() {
            return Ok(MemoryBrief::Absent);
        }
        let held = self.read()?;
        if format::main_kind(&held.content)? == format::Kind::Legacy {
            return Ok(MemoryBrief::Legacy {
                revision: held.revision,
            });
        }
        let sections = format::section_bodies(&held.content, keys)?
            .into_iter()
            .map(|(key, body)| (key.to_string(), preview(body, max_bytes)))
            .collect();
        Ok(MemoryBrief::Structured {
            revision: held.revision,
            sections,
        })
    }
}

fn preview(text: &str, max_bytes: usize) -> TextPreview {
    let bytes = text.len();
    if bytes <= max_bytes {
        return TextPreview {
            text: text.to_string(),
            bytes,
            truncated: false,
        };
    }
    let mut end = max_bytes;
    while end > 0 && !text.is_char_boundary(end) {
        end -= 1;
    }
    TextPreview {
        text: text[..end].to_string(),
        bytes,
        truncated: true,
    }
}
