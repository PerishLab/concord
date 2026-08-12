use super::Audit;
use crate::Result;
use crate::memory::format;
use std::path::Path;

pub(super) fn inspect(audit: &mut Audit, root: &Path) -> Result<()> {
    let primary = root.join("MAIN.md");
    let structured = main(audit, &primary)?;
    let phases = root.join("phases");
    if !phases.exists() {
        return Ok(());
    }
    if !phases.is_dir() {
        audit.fault("memory", &phases, "memory phases path is not a directory");
        return Ok(());
    }
    let mut numbered = Vec::new();
    for entry in std::fs::read_dir(&phases)? {
        let entry = entry?;
        let path = entry.path();
        if !entry.file_type()?.is_file() {
            audit.fault("memory", &path, "memory phase entry is not a regular file");
            continue;
        }
        let Some(filename) = entry.file_name().to_str().map(str::to_string) else {
            audit.fault("memory", &path, "memory phase name is not UTF-8");
            continue;
        };
        let Some(number) = number(&filename) else {
            audit.fault("memory", &path, "malformed memory phase name");
            continue;
        };
        numbered.push((number, path));
    }
    numbered.sort_by_key(|(number, _)| *number);
    for (expected, (number, path)) in numbered.iter().enumerate() {
        if *number != expected as u32 {
            audit.fault(
                "memory",
                path,
                format!(
                    "memory phases must be contiguous from 0; expected {expected}, found {number}"
                ),
            );
        }
        phase(audit, path, structured)?;
    }
    if numbered.len() >= 16 {
        audit.observe(
            "memory",
            &phases,
            format!(
                "{} phases retained; consider a smaller follow-up task or deliberate compaction",
                numbered.len()
            ),
        );
    }
    Ok(())
}

fn main(audit: &mut Audit, path: &Path) -> Result<bool> {
    let Ok(metadata) = std::fs::symlink_metadata(path) else {
        return Ok(false);
    };
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Ok(false);
    }
    let Some(content) = text(audit, path, format::limit(format::Schema::Main), "MAIN.md")? else {
        return Ok(false);
    };
    match format::main(&content) {
        Ok(_) => match format::kind(&content, format::Schema::Main) {
            Ok(kind) => Ok(kind == format::Kind::V1),
            Err(error) => {
                audit.fault("memory", path, format!("{}: {error}", error.code()));
                Ok(false)
            }
        },
        Err(error) => {
            audit.fault("memory", path, format!("{}: {error}", error.code()));
            Ok(false)
        }
    }
}

fn phase(audit: &mut Audit, path: &Path, structured: bool) -> Result<()> {
    let Some(content) = text(audit, path, format::limit(format::Schema::Phase), "phase")? else {
        return Ok(());
    };
    if let Err(error) = format::phase(&content, structured) {
        audit.fault("memory", path, format!("{}: {error}", error.code()));
    }
    Ok(())
}

fn text(report: &mut Audit, path: &Path, max_bytes: usize, label: &str) -> Result<Option<String>> {
    let metadata = std::fs::metadata(path)?;
    if metadata.len() > max_bytes as u64 {
        report.fault(
            "memory",
            path,
            format!(
                "{label} is {} bytes; maximum is {max_bytes}",
                metadata.len()
            ),
        );
        return Ok(None);
    }
    let bytes = std::fs::read(path)?;
    match String::from_utf8(bytes) {
        Ok(content) => Ok(Some(content)),
        Err(_) => {
            report.fault("memory", path, format!("{label} is not UTF-8"));
            Ok(None)
        }
    }
}

fn number(name: &str) -> Option<u32> {
    let digits = name.strip_prefix("PHASE-")?.strip_suffix(".md")?;
    if digits.len() < 2 || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    let number = digits.parse::<u32>().ok()?;
    (format!("PHASE-{number:02}.md") == name).then_some(number)
}
