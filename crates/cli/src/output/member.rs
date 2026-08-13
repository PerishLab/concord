use concord_core::MemberStatus;

pub fn status(status: &MemberStatus) {
    println!(
        "member status: {}/{}",
        status.member.task, status.member.name
    );
    checkout("worktree", &status.worktree);
    checkout("integration", &status.integration_checkout);
    println!("  boundary: {}", status.boundary.name());
    println!("  integration relation: {}", status.integration.name());
    match &status.local_upstream {
        Some(upstream) => println!(
            "  local upstream: {} {} ahead={} behind={}",
            upstream.reference, upstream.head, upstream.ahead, upstream.behind
        ),
        None => println!("  local upstream: -"),
    }
    if status.local_tracking_refs.is_empty() {
        println!("  local tracking refs: -");
    } else {
        println!(
            "  local tracking refs: {}",
            status.local_tracking_refs.join(" ")
        );
    }
}

fn checkout(name: &str, state: &concord_core::CheckoutState) {
    println!("  {name}: {}", state.path);
    println!("    head: {}", state.head);
    println!(
        "    files: clean={} tracked={} untracked={}",
        state.clean, state.tracked_changes, state.untracked_files
    );
}
