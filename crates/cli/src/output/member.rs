use concord_core::Status;

pub fn status(status: &Status) {
    if !super::line(format_args!(
        "member status: {}",
        status.member.issue.identity()
    )) {
        return;
    }
    if status.references.is_empty() {
        if !super::line("  references: -") {
            return;
        }
    } else {
        for reference in &status.references {
            if !super::line(format_args!(
                "  reference: {} {}/{}/{}#{}",
                reference.kind.name(),
                reference.provider,
                reference.owner,
                reference.repository,
                reference.number
            )) {
                return;
            }
        }
    }
    if !checkout("worktree", &status.worktree) {
        return;
    }
    if !checkout("integration", &status.checkout) {
        return;
    }
    if !super::line(format_args!("  boundary: {}", status.boundary.name())) {
        return;
    }
    if !super::line(format_args!(
        "  integration relation: {}",
        status.integration.name()
    )) {
        return;
    }
    let ok = match &status.upstream {
        Some(upstream) => super::line(format_args!(
            "  local upstream: {} {} ahead={} behind={}",
            upstream.reference, upstream.head, upstream.ahead, upstream.behind
        )),
        None => super::line("  local upstream: -"),
    };
    if !ok {
        return;
    }
    if status.tracking.is_empty() {
        let _ = super::line("  local tracking refs: -");
    } else {
        let _ = super::line(format_args!(
            "  local tracking refs: {}",
            status.tracking.join(" ")
        ));
    }
}

fn checkout(name: &str, state: &concord_core::Checkout) -> bool {
    super::line(format_args!("  {name}: {}", state.path))
        && super::line(format_args!("    head: {}", state.head))
        && super::line(format_args!(
            "    files: clean={} tracked={} untracked={}",
            state.clean, state.tracked, state.untracked
        ))
}
