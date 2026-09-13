use concord_core::{Error, Result, Root};
use plumb::config::Cascade;
use std::path::{Path, PathBuf};

const RELEASES: &str = "https://releases.concord.perish.uk";
const DEPOT: &str = "https://depot.concord.perish.uk";

#[derive(Clone, Debug, Cascade)]
pub struct Config {
    pub domain_space_root: PathBuf,
    pub home: PathBuf,
    pub releases: String,
    pub depot: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            domain_space_root: PathBuf::new(),
            home: plumb::config::data("concord").unwrap_or_default(),
            releases: RELEASES.to_string(),
            depot: DEPOT.to_string(),
        }
    }
}

impl Config {
    pub fn load(
        file: Option<&Path>,
        root: Option<&Path>,
        home: Option<&Path>,
        releases: Option<&str>,
    ) -> Result<Self> {
        let selected = selected(file)?;
        let config = Self::resolve_with(
            selected.as_deref(),
            ConfigPartial {
                domain_space_root: root.map(Path::to_path_buf),
                home: home.map(Path::to_path_buf),
                releases: releases.map(str::to_string),
                depot: None,
            },
        )
        .map_err(|error| Error::new(error.to_string()))?;
        config.validate()
    }

    pub fn path(file: Option<&Path>) -> Result<PathBuf> {
        match file {
            Some(path) => absolute(path),
            None => default(),
        }
    }

    pub fn root(&self) -> Result<Root> {
        if self.domain_space_root.as_os_str().is_empty() {
            return Err(Error::new(
                "domain_space_root is required for task operations; set it in config, \
                 CONCORD_DOMAIN_SPACE_ROOT, or --root",
            ));
        }
        Root::new(&self.domain_space_root)
    }

    fn validate(self) -> Result<Self> {
        if self.home.as_os_str().is_empty() {
            return Err(Error::new(
                "Concord data home is unavailable; set home, CONCORD_HOME, or --home",
            ));
        }
        if !self.home.is_absolute() {
            return Err(Error::new("home must be absolute"));
        }
        if !self.releases.starts_with("https://") && !self.releases.starts_with("http://") {
            return Err(Error::new("releases must be an http or https URL"));
        }
        if !self.depot.starts_with("https://") && !self.depot.starts_with("http://") {
            return Err(Error::new("depot must be an http or https URL"));
        }
        Ok(self)
    }
}

fn selected(file: Option<&Path>) -> Result<Option<PathBuf>> {
    if file.is_some() {
        return Config::path(file).map(Some);
    }
    let path = default()?;
    Ok(path.is_file().then_some(path))
}

fn default() -> Result<PathBuf> {
    plumb::config::data("concord")
        .map(|home| home.join("concord.toml"))
        .ok_or_else(|| Error::new("platform data home is unavailable; pass --config"))
}

fn absolute(path: &Path) -> Result<PathBuf> {
    if path.is_absolute() {
        return Ok(path.to_path_buf());
    }
    Err(Error::new("--config must be an absolute path"))
}
