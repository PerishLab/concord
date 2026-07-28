use crate::{Error, Result};
use serde::Deserialize;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Deserialize)]
pub struct Config {
    pub domain_space_root: PathBuf,
}

#[derive(Clone, Debug)]
pub struct Root {
    path: PathBuf,
}

impl Config {
    pub fn load(override_root: Option<&Path>) -> Result<Root> {
        if let Some(root) = override_root {
            return Root::new(root);
        }
        if let Some(root) = environment("CONCORD_DOMAIN_SPACE_ROOT") {
            return Root::new(Path::new(&root));
        }
        let path = config_path()?;
        let text = std::fs::read_to_string(&path).map_err(|error| {
            Error::new(format!(
                "cannot read Concord config {}: {error}",
                path.display()
            ))
        })?;
        let config: Self = toml::from_str(&text)?;
        Root::new(&config.domain_space_root)
    }

    pub fn path() -> Result<PathBuf> {
        config_path()
    }
}

impl Root {
    pub fn new(path: &Path) -> Result<Self> {
        if !path.is_absolute() {
            return Err(Error::new("domain_space_root must be absolute"));
        }
        let path = path.canonicalize().map_err(|error| {
            Error::new(format!(
                "cannot resolve domain_space_root {}: {error}",
                path.display()
            ))
        })?;
        if !path.is_dir() {
            return Err(Error::new(format!(
                "domain_space_root is not a directory: {}",
                path.display()
            )));
        }
        Ok(Self { path })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

fn config_path() -> Result<PathBuf> {
    if let Some(path) = environment("CONCORD_CONFIG") {
        return Ok(PathBuf::from(path));
    }
    if let Some(root) = environment("XDG_CONFIG_HOME") {
        return Ok(PathBuf::from(root).join("concord/config.toml"));
    }
    if cfg!(windows) {
        let root = environment("APPDATA")
            .ok_or_else(|| Error::new("APPDATA is required to locate Concord config"))?;
        return Ok(PathBuf::from(root).join("concord/config.toml"));
    }
    let home = environment("HOME")
        .ok_or_else(|| Error::new("HOME is required to locate Concord config"))?;
    Ok(PathBuf::from(home).join(".config/concord/config.toml"))
}

pub(crate) fn home() -> Result<PathBuf> {
    environment("HOME")
        .map(PathBuf::from)
        .ok_or_else(|| Error::new("HOME is required to expand repository source"))
}

pub(crate) fn current_dir() -> Result<PathBuf> {
    Ok(std::env::current_dir()?.canonicalize()?)
}

fn environment(name: &str) -> Option<std::ffi::OsString> {
    std::env::var_os(name)
}
