use crate::output;
use concord_core::{Plan, Result};

pub(crate) fn create(
    plan: Plan,
    dry_run: bool,
    json_output: bool,
    apply: impl FnOnce() -> Result<Plan>,
) -> Result<()> {
    if dry_run {
        return output::plan(&plan, json_output);
    }
    if json_output {
        return output::plan(&apply()?, true);
    }
    output::plan(&plan, false)?;
    apply()?;
    output::applied(false);
    Ok(())
}

pub(crate) fn guarded(
    plan: Plan,
    apply_now: bool,
    json_output: bool,
    apply: impl FnOnce() -> Result<Plan>,
) -> Result<()> {
    if !apply_now {
        return output::plan(&plan, json_output);
    }
    if json_output {
        return output::plan(&apply()?, true);
    }
    output::plan(&plan, false)?;
    apply()?;
    output::applied(false);
    Ok(())
}
