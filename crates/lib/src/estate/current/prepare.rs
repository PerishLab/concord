use super::super::{Edit, Fact, Role};
use crate::{Error, Result};
use std::collections::{BTreeMap, BTreeSet};

pub(in crate::estate) struct Prepared {
    pub create: Vec<Fact>,
    pub set: Vec<Fact>,
    pub end: Vec<(Role, i64)>,
}

pub(in crate::estate) fn build(current: &[Fact], edits: &[Edit]) -> Result<Prepared> {
    let mut facts = current.to_vec();
    let mut touched = BTreeSet::new();
    let mut prepared = Prepared {
        create: Vec::new(),
        set: Vec::new(),
        end: Vec::new(),
    };
    for edit in edits {
        match edit {
            Edit::Create { fact } => {
                validate(fact, false)?;
                facts.push(fact.clone());
                prepared.create.push(fact.clone());
            }
            Edit::Set { fact } => {
                validate(fact, true)?;
                let key = fact.key.expect("validated key");
                touch(&mut touched, fact.role, key)?;
                replace(&mut facts, fact)?;
                prepared.set.push(fact.clone());
            }
            Edit::End { role, key } => {
                touch(&mut touched, *role, *key)?;
                remove(&mut facts, *role, *key)?;
                prepared.end.push((*role, *key));
            }
        }
    }
    finality(&facts)?;
    Ok(prepared)
}

pub(super) fn validate(fact: &Fact, held: bool) -> Result<()> {
    if held != fact.key.is_some() {
        return Err(Error::typed(
            "concord.change.key",
            "create omits a key and set requires one",
        ));
    }
    if fact.body.trim().is_empty() {
        return Err(Error::typed(
            "concord.change.body",
            "fact body cannot be blank",
        ));
    }
    if fact.role.singular() != fact.rank.is_none() {
        return Err(Error::typed(
            "concord.change.rank",
            "singular facts omit rank and ordered facts require rank",
        ));
    }
    if fact.rank.is_some_and(|rank| rank < 1) {
        return Err(Error::typed(
            "concord.change.rank",
            "fact rank must be positive",
        ));
    }
    let addition = fact.role == Role::Addition;
    if addition
        && fact
            .origin
            .as_ref()
            .is_none_or(|origin| origin.trim().is_empty())
    {
        return Err(Error::typed(
            "concord.change.origin",
            "Addition requires a nonblank origin",
        ));
    }
    if !addition && (fact.title.is_some() || fact.origin.is_some()) {
        return Err(Error::typed(
            "concord.change.shape",
            "title and origin belong only to Addition",
        ));
    }
    Ok(())
}

pub(in crate::estate) fn temporary(
    current: &[Fact],
    prepared: &Prepared,
) -> Result<BTreeMap<(Role, i64), i64>> {
    let mut next = current
        .iter()
        .chain(&prepared.create)
        .chain(&prepared.set)
        .filter_map(|fact| fact.rank)
        .max()
        .unwrap_or(0);
    let mut ranks = BTreeMap::new();
    for fact in &prepared.set {
        if !fact.role.singular() {
            next = next.checked_add(1).ok_or_else(|| {
                Error::typed(
                    "concord.change.rank",
                    "temporary rank exceeds integer range",
                )
            })?;
            ranks.insert((fact.role, fact.key.expect("set key")), next);
        }
    }
    Ok(ranks)
}

fn touch(touched: &mut BTreeSet<(Role, i64)>, role: Role, key: i64) -> Result<()> {
    if touched.insert((role, key)) {
        return Ok(());
    }
    Err(Error::typed(
        "concord.change.duplicate",
        format!("fact {key} is edited more than once"),
    ))
}

fn replace(facts: &mut [Fact], fact: &Fact) -> Result<()> {
    let key = fact.key.expect("validated key");
    let held = facts
        .iter_mut()
        .find(|held| held.key == Some(key) && held.role == fact.role)
        .ok_or_else(|| Error::typed("concord.change.absent", format!("fact {key} not found")))?;
    *held = fact.clone();
    Ok(())
}

fn remove(facts: &mut Vec<Fact>, role: Role, key: i64) -> Result<()> {
    let before = facts.len();
    facts.retain(|fact| fact.key != Some(key) || fact.role != role);
    if facts.len() != before {
        return Ok(());
    }
    Err(Error::typed(
        "concord.change.absent",
        format!("fact {key} not found"),
    ))
}

fn finality(facts: &[Fact]) -> Result<()> {
    for role in Role::ALL {
        let held = facts.iter().filter(|fact| fact.role == role);
        if role.singular() {
            if held.count() > 1 {
                return Err(Error::typed(
                    "concord.change.cardinality",
                    format!("{role:?} admits at most one fact"),
                ));
            }
            continue;
        }
        let mut ranks = BTreeSet::new();
        if held
            .filter_map(|fact| fact.rank)
            .any(|rank| !ranks.insert(rank))
        {
            return Err(Error::typed(
                "concord.change.rank",
                format!("{role:?} ranks must be unique"),
            ));
        }
    }
    Ok(())
}
