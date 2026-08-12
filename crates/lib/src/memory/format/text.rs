use super::Kind;
use crate::{Error, Result};

pub(super) fn marker(source: &str, exact: &str, family: &str) -> Result<Kind> {
    let first = first(source);
    let candidate = first.strip_prefix('\u{feff}').unwrap_or(first);
    let candidate = candidate.strip_suffix('\r').unwrap_or(candidate);
    if candidate == exact {
        return Ok(Kind::V1);
    }
    if candidate.starts_with("<!--") && candidate.contains(family) {
        return Err(Error::typed(
            "memory.unknown_version",
            format!("unsupported memory marker: {first}"),
        ));
    }
    Ok(Kind::Legacy)
}

pub(super) fn limits(content: &str, label: &str, max_bytes: usize, max_lines: usize) -> Result<()> {
    if content.len() > max_bytes {
        return Err(Error::typed(
            "memory.limit_bytes",
            format!("{label} is {} bytes; maximum is {max_bytes}", content.len()),
        ));
    }
    let lines = content.split_terminator('\n').count();
    if lines > max_lines {
        return Err(Error::typed(
            "memory.limit_lines",
            format!("{label} is {lines} lines; maximum is {max_lines}"),
        ));
    }
    Ok(())
}

pub(super) fn strict(content: &str, label: &str) -> Result<()> {
    if content.starts_with('\u{feff}') {
        return Err(Error::typed(
            "memory.text_bom",
            format!("{label} must not start with a UTF-8 BOM"),
        ));
    }
    if content.contains('\r') {
        return Err(Error::typed(
            "memory.text_newline",
            format!("{label} must use LF newlines"),
        ));
    }
    if !content.ends_with('\n') {
        return Err(Error::typed(
            "memory.text_final_newline",
            format!("{label} must end with a final newline"),
        ));
    }
    Ok(())
}

pub(super) fn first(source: &str) -> &str {
    source.split_once('\n').map_or(source, |(line, _)| line)
}
