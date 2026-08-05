use concord_core::{Error, MemoryBrief, Result, TaskBriefPage, TextPreview};
use std::collections::BTreeMap;

pub fn brief(page: &TaskBriefPage, json_output: bool) -> Result<()> {
    if json_output {
        println!(
            "{}",
            serde_json::to_string_pretty(page)
                .map_err(|error| Error::new(format!("cannot encode task brief: {error}")))?
        );
        return Ok(());
    }
    println!(
        "task brief: {} ({}/{}, max {} tasks, {} bytes/section)",
        page.domain, page.returned, page.total, page.limits.tasks, page.limits.section_bytes
    );
    for entry in &page.tasks {
        println!("  {}", entry.identity);
        memory(&entry.memory);
    }
    if let Some(next) = &page.next {
        println!("next: --after {next}");
    }
    Ok(())
}

fn memory(brief: &MemoryBrief) {
    match brief {
        MemoryBrief::Absent => println!("    memory: absent"),
        MemoryBrief::Legacy { revision } => println!("    memory: legacy {revision}"),
        MemoryBrief::Unavailable { code, message } => {
            println!("    memory: unavailable {code}: {message}")
        }
        MemoryBrief::Structured { revision, sections } => structured(revision, sections),
    }
}

fn structured(revision: &str, sections: &BTreeMap<String, TextPreview>) {
    println!("    memory: structured {revision}");
    for key in ["focus", "next"] {
        if let Some(section) = sections.get(key) {
            let suffix = if section.truncated {
                " (truncated)"
            } else {
                ""
            };
            println!("    {key}{suffix}: {}", section.text);
        }
    }
}
