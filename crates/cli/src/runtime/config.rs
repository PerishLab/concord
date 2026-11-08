mod legacy;

use concord_core::{Error, Result};
use radicula_config::Cascade;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

const RELEASES: &str = "https://releases.concord.perish.uk";
const DEPOT: &str = "https://depot.concord.perish.uk";

#[derive(Clone, Debug, Cascade)]
#[plumb::config::prefix]
pub struct Config {
    pub home: PathBuf,
    pub releases: String,
    pub depot: String,
    pub phase: Option<crate::phase::Binding>,
}

pub struct Loaded {
    config: Config,
    selected: Source,
}

#[derive(Clone)]
enum Source {
    Explicit,
    Environment,
    Config(File),
    Default,
}

#[derive(Clone)]
struct File {
    path: PathBuf,
    explicit: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            home: plumb::config::data("concord").unwrap_or_default(),
            releases: RELEASES.to_string(),
            depot: DEPOT.to_string(),
            phase: None,
        }
    }
}

impl Config {
    pub fn load(
        file: Option<&Path>,
        root: Option<&Path>,
        home: Option<&Path>,
        releases: Option<&str>,
    ) -> Result<Loaded> {
        legacy::environment()?;
        if root.is_some() {
            return Err(Error::typed(
                "concord.config.legacy",
                "--space no longer selects local execution; migrate to the data home layout and remove --space; inspect concord config show",
            ));
        }
        let file = selected(file)?;
        let mut config = Self::default();
        let mut source = Source::Default;
        if let Some(file) = &file {
            let seen = legacy::load(&file.path)?;
            if seen.home.is_some() {
                source = Source::Config(file.clone());
            }
            config = config.merge(seen);
        }
        let environment = <Self as Cascade>::lookup(&Self::prefix(), &plumb::config::value)
            .map_err(|error| Error::new(error.to_string()))?;
        if environment.home.is_some() {
            source = Source::Environment;
        }
        config = config.merge(environment);
        if home.is_some() {
            source = Source::Explicit;
        }
        config = config.merge(ConfigPartial {
            home: home.map(Path::to_path_buf),
            releases: releases.map(str::to_string),
            depot: None,
            phase: None,
        });
        let config = config.validate()?;
        Ok(Loaded {
            config,
            selected: source,
        })
    }

    pub fn path(file: Option<&Path>) -> Result<PathBuf> {
        match file {
            Some(path) => absolute(path),
            None => default(),
        }
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

impl Loaded {
    pub fn config(&self) -> &Config {
        &self.config
    }
    pub fn provenance(&self) -> Value {
        self.selected.json()
    }
    pub fn absent(&self, error: Error) -> Error {
        if error.code() != "concord.estate.absent" {
            return error;
        }
        let estate = self.config.home.join("workspace");
        Error::detailed(
            "concord.estate.absent",
            format!(
                "Concord Space is absent at {}; inspect concord config show; bootstrap only an intentionally new data home, and migrate existing execution explicitly",
                estate.display()
            ),
            json!({"home": self.config.home.display().to_string(), "estate": estate.display().to_string(), "provenance": self.provenance()}),
        )
    }
}

impl Source {
    fn json(&self) -> Value {
        match self {
            Self::Explicit => json!({"kind":"explicit", "input":"--home"}),
            Self::Environment => json!({"kind":"environment", "input":"CONCORD_HOME"}),
            Self::Config(file) => {
                json!({"kind":"config", "input": file.input(), "path":file.path.display().to_string()})
            }
            Self::Default => json!({"kind":"built-in-default"}),
        }
    }
}

fn selected(file: Option<&Path>) -> Result<Option<File>> {
    if let Some(file) = file {
        return Config::path(Some(file)).map(|path| {
            Some(File {
                path,
                explicit: true,
            })
        });
    }
    let path = default()?;
    Ok(path.is_file().then_some(File {
        path,
        explicit: false,
    }))
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

impl File {
    fn input(&self) -> &str {
        if self.explicit {
            "--config"
        } else {
            "platform-default"
        }
    }
}
