use super::super::super::super::{Part, Role};
use crate::memory::format;
use crate::{Entry, Fact, Result};

pub(in crate::estate::migration) fn facts(source: &str) -> Result<Vec<Fact>> {
    if format::kind(source, format::Schema::Main)? == format::Kind::Legacy {
        let body = source.trim();
        return Ok((!body.is_empty())
            .then(|| Fact {
                key: None,
                role: Role::Addition,
                rank: Some(1),
                title: Some("MAIN.md".to_string()),
                body: body.to_string(),
                origin: Some("legacy-main".to_string()),
            })
            .into_iter()
            .collect());
    }
    let extracted = format::import(source, format::Schema::Main)?;
    let mut held = Vec::new();
    for (key, body) in extracted.sections {
        if body.is_empty() {
            continue;
        }
        let (role, rank) = match key {
            "goal" => (Role::Goal, None),
            "constraints" => (Role::Constraint, Some(1)),
            "decisions" => (Role::Decision, Some(1)),
            "focus" => (Role::Focus, None),
            "questions" => (Role::Question, Some(1)),
            "next" => (Role::Next, None),
            _ => unreachable!("fixed MAIN section"),
        };
        held.push(Fact {
            key: None,
            role,
            rank,
            title: None,
            body: body.to_string(),
            origin: None,
        });
    }
    if let Some(body) = extracted.preamble {
        held.push(Fact {
            key: None,
            role: Role::Addition,
            rank: Some(1),
            title: Some("MAIN.md preamble".to_string()),
            body: body.to_string(),
            origin: Some("legacy-main".to_string()),
        });
    }
    Ok(held)
}

pub(in crate::estate::migration) fn entries(source: &str) -> Result<Vec<Entry>> {
    if format::kind(source, format::Schema::Phase)? == format::Kind::Legacy {
        let body = source.trim();
        return Ok((!body.is_empty())
            .then(|| Entry {
                key: None,
                part: Part::Addition,
                rank: Some(1),
                title: Some("PHASE preamble".to_string()),
                body: body.to_string(),
                origin: Some("legacy-phase".to_string()),
            })
            .into_iter()
            .collect());
    }
    let extracted = format::import(source, format::Schema::Phase)?;
    let mut held = Vec::new();
    for (key, body) in extracted.sections {
        if body.is_empty() {
            continue;
        }
        let (part, rank) = match key {
            "outcome" => (Part::Outcome, None),
            "decisions" => (Part::Decision, Some(1)),
            "evidence" => (Part::Evidence, Some(1)),
            "carry-forward" => (Part::Carry, Some(1)),
            _ => unreachable!("fixed Phase section"),
        };
        held.push(Entry {
            key: None,
            part,
            rank,
            title: None,
            body: body.to_string(),
            origin: None,
        });
    }
    if let Some(body) = extracted.preamble {
        held.push(Entry {
            key: None,
            part: Part::Addition,
            rank: Some(1),
            title: Some("PHASE preamble".to_string()),
            body: body.to_string(),
            origin: Some("legacy-phase".to_string()),
        });
    }
    Ok(held)
}
