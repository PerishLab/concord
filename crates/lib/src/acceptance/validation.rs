use super::{Judgment, Marker, Reference, Target};
use crate::Result;

pub(super) fn validate(marker: &Marker) -> Result<()> {
    match marker {
        Marker::Declaration { promise, .. } => text(promise),
        Marker::Amendment {
            promise,
            predecessor,
            reason,
            ..
        } => {
            text(promise)?;
            reference(predecessor)?;
            text(reason)
        }
        Marker::Closure { .. } => closure(marker),
    }
}

fn closure(marker: &Marker) -> Result<()> {
    let Marker::Closure {
        target,
        declaration,
        judgment,
        review,
        evidence,
        remaining,
        release,
        ..
    } = marker
    else {
        unreachable!("closure validation selects a closure marker")
    };
    reference(declaration)?;
    fingerprint(review)?;
    entries(evidence)?;
    entries(remaining)?;
    match judgment {
        Judgment::Satisfied if evidence.is_empty() || !remaining.is_empty() => {
            return Err(super::fault(
                "satisfied closure requires evidence and no remaining obligations",
            ));
        }
        Judgment::Unmet if remaining.is_empty() => {
            return Err(super::fault(
                "unmet closure must name remaining obligations",
            ));
        }
        _ => {}
    }
    if let Some(release) = release {
        if *target != Target::Release {
            return Err(super::fault(
                "source closure cannot claim release verification",
            ));
        }
        text(&release.marker)?;
        text(&release.distribution)?;
        entries(&release.inclusion)?;
        if release.inclusion.is_empty() {
            return Err(super::fault(
                "release verification requires reviewed inclusion evidence",
            ));
        }
    } else if *target == Target::Release && *judgment == Judgment::Satisfied {
        return Err(super::fault(
            "satisfied release closure requires release verification",
        ));
    }
    Ok(())
}

fn reference(reference: &Reference) -> Result<()> {
    text(&reference.node)?;
    fingerprint(&reference.digest)
}

fn fingerprint(value: &str) -> Result<()> {
    if value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Ok(());
    }
    Err(super::fault(
        "acceptance reference requires a lowercase SHA-256 digest",
    ))
}

fn entries(values: &[String]) -> Result<()> {
    let mut seen = std::collections::BTreeSet::new();
    for value in values {
        text(value)?;
        if !seen.insert(value) {
            return Err(super::fault(
                "acceptance evidence or obligations contain duplicate entries",
            ));
        }
    }
    Ok(())
}

fn text(value: &str) -> Result<()> {
    if !value.trim().is_empty() && !value.contains("-->") {
        return Ok(());
    }
    Err(super::fault(
        "acceptance text is empty or contains an HTML comment terminator",
    ))
}
