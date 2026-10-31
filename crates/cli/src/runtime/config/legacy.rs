use super::ConfigPartial;
use concord_core::{Error, Result};
use serde::Deserialize;
use serde::de::IgnoredAny;
use std::path::Path;

#[derive(Deserialize)]
struct Layer {
    #[serde(rename = "domain_space_root")]
    legacy: Option<IgnoredAny>,
    #[serde(flatten)]
    partial: ConfigPartial,
}

pub(super) fn environment() -> Result<()> {
    if std::env::var_os("CONCORD_DOMAIN_SPACE_ROOT").is_some() {
        return Err(Error::typed(
            "concord.config.legacy",
            "CONCORD_DOMAIN_SPACE_ROOT is no longer supported; rename it to CONCORD_SPACE",
        ));
    }
    Ok(())
}

pub(super) fn load(path: &Path) -> Result<ConfigPartial> {
    let layer =
        plumb::config::load::<Layer>(path).map_err(|error| Error::new(error.to_string()))?;
    if layer.legacy.is_some() {
        return Err(Error::typed(
            "concord.config.legacy",
            format!(
                "config {} sets domain_space_root; rename that key to space",
                path.display(),
            ),
        ));
    }
    Ok(layer.partial)
}
