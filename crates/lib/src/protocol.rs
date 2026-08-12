pub(crate) mod claim;

use crate::{Error, Result};

pub const PLUMB: &str = env!("CONCORD_PLUMB_VERSION");

pub(crate) fn component(label: &str, value: &str) -> Result<()> {
    if value.is_empty() || value == "." || value == ".." {
        return Err(Error::new(format!("{label} is not a path component")));
    }
    if value.contains('/') || value.contains('\\') || value.contains('\0') {
        return Err(Error::new(format!(
            "{label} is not a single path component"
        )));
    }
    Ok(())
}
