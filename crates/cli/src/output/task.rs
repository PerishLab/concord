use concord_core::{RoleBrief, TaskBriefEntry, TaskBriefPage};

pub fn brief(page: &TaskBriefPage) {
    println!(
        "task brief: {} ({}/{}, max {} tasks, {} bytes/role)",
        page.domain, page.returned, page.total, page.limits.tasks, page.limits.role_bytes
    );
    for task in &page.tasks {
        entry(task, &page.limits.roles);
    }
    if let Some(next) = &page.next {
        println!("next: --after {next}");
    }
}

fn entry(task: &TaskBriefEntry, roles: &[&str]) {
    println!("  {} revision {}", task.identity, task.task.revision);
    for name in roles {
        if let Some(brief) = task.roles.get(name) {
            role(name, brief);
        }
    }
}

fn role(name: &str, brief: &RoleBrief) {
    for fact in &brief.facts {
        let suffix = if fact.body.truncated {
            " (truncated)"
        } else {
            ""
        };
        println!("    {name}{suffix}: {}", line(&fact.body.text));
    }
}

fn line(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}
