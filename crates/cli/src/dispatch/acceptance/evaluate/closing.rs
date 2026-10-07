use concord_core::acceptance::digest;
use concord_core::{Coordinate, Result};
use serde_json::json;

use super::{Projection, pull};
use crate::args::acceptance::{Closing, Observe};

pub(in crate::dispatch::acceptance) async fn run(args: Closing, output: bool) -> Result<()> {
    if !(1..=20).contains(&args.observe.pages) || !(1..=120).contains(&args.observe.timeout) {
        return Err(fault(
            "bounds",
            "closing-reference reads require 1..20 pages and a 1..120 second timeout",
        ));
    }
    if args.head.len() != 40
        || !args
            .head
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(fault(
            "head",
            "CI head must be an explicit lowercase 40-digit Git commit",
        ));
    }
    let coordinate = Coordinate::parse(&args.observe.issue)?;
    let reader = pull::Reader {
        args: &args,
        coordinate: &coordinate,
    };
    let pull = reader.complete().await?;
    if reader.complete().await? != pull {
        return Err(fault(
            "changed",
            "PR closing references changed during initial observation",
        ));
    }
    let first = evaluate(&args.observe, &pull.references).await?;
    if reader.complete().await? != pull {
        return Err(fault(
            "changed",
            "PR head or references changed during Issue evaluation",
        ));
    }
    let issues = evaluate(&args.observe, &pull.references).await?;
    if issues != first || reader.complete().await? != pull {
        return Err(fault(
            "changed",
            "closing-reference facts moved during final observation",
        ));
    }
    let passed = issues.iter().all(Projection::accepted);
    let mut report = json!({
        "schema": "concord.acceptance-closing/v1", "pull": pull, "issues": issues, "passed": passed,
        "semantics": "bounded live observations, not a provider transaction, merge authorization, automatic closure or authenticated verification identity"
    });
    let encoded =
        serde_json::to_string(&report).map_err(|error| fault("encode", error.to_string()))?;
    report["digest"] = digest(&encoded).into();
    crate::dispatch::emit(json!({"closing": report}), output)?;
    if passed {
        Ok(())
    } else {
        Err(fault(
            "closing",
            "one or more current closing Issues have unmet or unknown acceptance; see the emitted report",
        ))
    }
}

async fn evaluate(args: &Observe, links: &[pull::Link]) -> Result<Vec<Projection>> {
    let mut issues = Vec::new();
    let mut bytes = 0;
    for link in links {
        let observe = Observe {
            issue: format!(
                "{}/{}#{}",
                link.coordinate.owner, link.coordinate.repository, link.coordinate.number
            ),
            command: args.command.clone(),
            timeout: args.timeout,
            pages: args.pages,
        };
        let projection = super::read(&observe).await?;
        if projection.snapshot.header.node != link.node
            || projection.snapshot.header.coordinate != link.coordinate
        {
            return Err(fault(
                "identity",
                "evaluated Issue differs from the native closing reference",
            ));
        }
        bytes += serde_json::to_vec(&projection)
            .map_err(|error| fault("encode", error.to_string()))?
            .len();
        if bytes > 8 * 1024 * 1024 {
            return Err(fault(
                "bounds",
                "closing-reference Issue observations exceed their retained byte bound",
            ));
        }
        issues.push(projection);
    }
    Ok(issues)
}

fn fault(kind: &str, message: impl Into<String>) -> concord_core::Error {
    crate::dispatch::acceptance::fault(kind, message)
}
