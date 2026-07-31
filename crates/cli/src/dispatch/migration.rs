use concord_core::{Error, MigrationClaim, Result};

pub fn claims(values: &[String]) -> Result<Vec<MigrationClaim>> {
    values.iter().map(|value| claim(value)).collect()
}

fn claim(value: &str) -> Result<MigrationClaim> {
    let (owner, write) = value.split_once('=').ok_or_else(|| invalid(value))?;
    let (task, member) = owner.rsplit_once('/').ok_or_else(|| invalid(value))?;
    Ok(MigrationClaim {
        task: task.to_string(),
        member: member.to_string(),
        write: write.to_string(),
    })
}

fn invalid(value: &str) -> Error {
    Error::new(format!("migration claim must be TASK/MEMBER=PATH: {value}"))
}
