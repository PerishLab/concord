use concord_core::{Footprint, TaskResources};

pub fn human(resources: &TaskResources) {
    println!(
        "  resources: {} {}",
        resources.status.label(),
        resources.identity
    );
    footprint(&resources.task, 4);
    for member in &resources.members {
        footprint(member, 4);
    }
    if let Some(memory) = &resources.memory {
        footprint(memory, 4);
    }
    for resource in &resources.resources {
        footprint(resource, 4);
    }
    match &resources.filesystem.value {
        Some(filesystem) => {
            println!(
                "    filesystem: {} {}% used, {} available of {}",
                filesystem.status.label(),
                filesystem.used_percent,
                bytes(filesystem.available_bytes),
                bytes(filesystem.total_bytes)
            );
            match &filesystem.inodes {
                Some(inodes) => println!(
                    "    inodes: {} {}% used, {} available of {}",
                    inodes.status.label(),
                    inodes.used_percent,
                    inodes.available,
                    inodes.total
                ),
                None => println!(
                    "    inodes: UNKNOWN {}",
                    filesystem
                        .inode_note
                        .as_deref()
                        .unwrap_or("capacity unavailable")
                ),
            }
        }
        None => println!(
            "    filesystem: UNKNOWN {}",
            resources
                .filesystem
                .note
                .as_deref()
                .unwrap_or("capacity unavailable")
        ),
    }
    match &resources.host_memory.value {
        Some(memory) => println!(
            "    memory: {} {}% available, {} of {}; swap {} used of {}",
            memory.status.label(),
            memory.available_percent,
            bytes(memory.available_bytes),
            bytes(memory.total_bytes),
            bytes(memory.swap_used_bytes),
            bytes(memory.swap_total_bytes)
        ),
        None => println!(
            "    memory: UNKNOWN {}",
            resources
                .host_memory
                .note
                .as_deref()
                .unwrap_or("capacity unavailable")
        ),
    }
}

fn footprint(footprint: &Footprint, indent: usize) {
    let padding = " ".repeat(indent);
    match (footprint.bytes, footprint.entries) {
        (Some(bytes_value), Some(entries)) => println!(
            "{padding}{}: {} {} across {} entries ({})",
            footprint.name,
            footprint.status.label(),
            bytes(bytes_value),
            entries,
            footprint.path
        ),
        _ => println!(
            "{padding}{}: UNKNOWN {} ({})",
            footprint.name,
            footprint.note.as_deref().unwrap_or("size unavailable"),
            footprint.path
        ),
    }
}

fn bytes(value: u64) -> String {
    const UNITS: [(&str, u64); 4] = [
        ("TiB", 1024_u64.pow(4)),
        ("GiB", 1024_u64.pow(3)),
        ("MiB", 1024_u64.pow(2)),
        ("KiB", 1024),
    ];
    for (unit, size) in UNITS {
        if value >= size {
            return format!("{:.1} {unit}", value as f64 / size as f64);
        }
    }
    format!("{value} B")
}
