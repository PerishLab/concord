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
            "concord.boundary.refused",
            "A Member Claim or Boundary proof does not authorize the observed committed delta.",
            "Inspect the Member's normalized Claim and committed diff. Narrow the change or explicitly revise the Claim, then create a fresh Boundary proof against the resulting HEAD.",
            (
                "Record the exact base, head, Claim digest, and paths named by the refusal.",
                "The fresh Boundary proof succeeds for the current Member HEAD and Claim.",
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
