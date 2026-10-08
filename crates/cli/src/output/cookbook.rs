use concord_core::{Error, Result};
use plumb::cookbook::{Cookbook, Entry};
use serde_json::json;

pub(crate) fn run(code: Option<&str>, json: bool) -> Result<()> {
    let book = book();
    let entries = match code {
        Some(code) => vec![book.get(code).ok_or_else(|| {
            Error::typed(
                "concord.cookbook.absent",
                format!("Cookbook entry does not exist: {code}"),
            )
        })?],
        None => book.entries().iter().collect(),
    };
    if json {
        return crate::dispatch::emit(json!({"cookbook": {"entries": entries}}), true);
    }
    for (index, entry) in entries.iter().enumerate() {
        if index > 0 {
            println!();
        }
        print(entry);
    }
    Ok(())
}

pub(crate) fn reference(code: &str) -> Option<String> {
    book()
        .get(code)
        .map(|entry| entry.code().text().to_string())
}

fn book() -> Cookbook {
    Cookbook::new([
        entry(
            "concord.issue.automation",
            "The selected Auto Issue is automation-held and has no Concord Anchor or Member.",
            "Read its operation and state through Issue projections. Plumb owns follow, verification, recovery and closure. Diagnose the recorded blocker and rerun its owner; any business-source repair belongs to an ordinary typed Issue and its Member.",
            (
                "Read the exact Auto Issue, linked pull, stopped-step evidence and native blockers without attaching it.",
                "The owning automation resumes within its registered paths, or an ordinary Issue carries the required human repair.",
            ),
        ),
        entry(
            "concord.boundary.refused",
            "A Member Claim or Boundary proof does not authorize the observed committed delta.",
            "Inspect the Member's normalized Claim and committed diff. Narrow the change or explicitly revise the Claim, then create a fresh Boundary proof against the resulting HEAD.",
            (
                "Record the exact base, head, Claim digest, and paths named by the refusal.",
                "The fresh Boundary proof succeeds for the current Member HEAD and Claim.",
            ),
        ),
        entry(
            "concord.delivery.branches",
            "The delivering repository holds a local or origin branch outside its Issue branch set: `main`, `release/vX.Y.Z`, automation-held `auto/<number>` refs, the branches of live Members in that repository and their `land/` projections, and `<type>/<number>` of an open Issue of that type in that repository.",
            "Delete each named branch, or give work worth keeping an open Issue and rename the branch to `<type>/<number>`, then re-run the refused command. Concord never deletes these branches itself and inspects no other repository.",
            (
                "Read the refusal's branch list, with each branch's place and reason, and inspect `git log main..<branch>` before deleting anything.",
                "The repository holds only branches in the Issue branch set and delivery proceeds past the branch check.",
            ),
        ),
        entry(
            "concord.delivery.landed",
            "After a squash merge, the fetched base head differs from the verified candidate in parent, tree or authority evidence. Native delivery also refuses lost or contradictory native proof.",
            "Do not blindly retry a completed merge. Compare the exact head and candidate under the selected authority, reverify what main actually holds, and land any repair as a new Issue-led change. Never fabricate a Guard proof for native delivery or force Member release past failed readback. Synchronize the integration checkout through the verified lifecycle.",
            (
                "Retain the refusal, which names the pull, head, candidate and kind, and inspect `git show <head>` against `git show <candidate>`.",
                "Main holds the exact authority-verified tree and evidence, and the integration checkout equals fetched main.",
            ),
        ),
        entry(
            "concord.delivery.native",
            "The explicit Wharf native gate failed, exceeded its budget, observed a changed tool world, or was used for a Plumb-governed source or base.",
            "Inspect the named failure in the exact clean Wharf Member. Fix the gate or source through its Issue, prove the updated Boundary, and explicitly prepare again. Do not change the fixed gate command, inherit provider credentials, or fall back from Plumb Guard.",
            (
                "Retain the gate refusal, exact source/base, Python identity and current Member Boundary.",
                "The fixed credential-free gate succeeds and a fresh native plan binds the current exact source and tool world.",
            ),
        ),
        entry(
            "concord.delivery.provider",
            "The explicit external provider command could not complete or confirm a pull-request mutation.",
            "Observe the exact repository, head branch, and pull request on GitHub before retrying. If the mutation already happened, resume from that provider state; otherwise fix the provider command or authentication and re-run the prepared delivery.",
            (
                "Retain the provider stderr and inspect the exact pull-request coordinate and current head OID.",
                "GitHub reports the intended pull request at the prepared head and Concord can confirm it.",
            ),
        ),
        entry(
            "concord.estate.upgrade_required",
            "The local control database does not match the exact Issue-execution estate understood by this Concord binary.",
            "Stop Concord writes. Preserve valuable source changes through Git branches or pull requests, remove the old private Concord control state and legacy seats, then run `concord issue bootstrap` and reattach only work justified by an open GitHub Issue.",
            (
                "Inspect registered Git worktrees and committed heads before removing private control state; do not retain a compatibility archive.",
                "A fresh Issue estate opens and `concord audit` agrees.",
            ),
        ),
        entry(
            "concord.issue.needs",
            "The operated GitHub Issue carries one or more labels in the `needs:` namespace, such as `needs:revalidation`, so Concord refuses to start or deliver it.",
            "Resolve what each named label asks for on the Issue itself, then remove those labels on GitHub and re-run the refused command. Concord never applies or removes these labels; labels on a parent Issue and labels outside the `needs:` namespace do not count.",
            (
                "Read the labels named by the refusal and the Issue's history that explains why each was applied.",
                "The Issue carries no `needs:` label and the command proceeds past observation.",
            ),
        ),
    ])
    .expect("compiled Concord Cookbook must be valid")
}

fn entry(code: &str, trigger: &str, solution: &str, recovery: (&str, &str)) -> Entry {
    Entry::new(code, trigger, solution)
        .and_then(|entry| entry.observe(recovery.0))
        .and_then(|entry| entry.retire(recovery.1))
        .expect("compiled Concord Cookbook entry must be valid")
}

fn print(entry: &Entry) {
    println!("{}", entry.code());
    println!("  trigger: {}", entry.trigger());
    println!("  solution: {}", entry.solution());
    println!("  evidence: {}", entry.evidence().expect("entry evidence"));
    println!("  exit: {}", entry.exit().expect("entry exit"));
}

#[cfg(test)]
mod tests {
    use super::{book, reference};

    #[test]
    fn resolution() {
        let book = book();
        for entry in book.entries() {
            let code = entry.code().text();
            assert_eq!(reference(code).as_deref(), Some(code));
            assert!(entry.evidence().is_some());
            assert!(entry.exit().is_some());
        }
        assert!(reference("concord.error").is_none());
    }
}
