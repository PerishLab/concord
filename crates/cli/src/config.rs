use concord_core::{Error, Result, Root};
use plumb::config::Cascade;
use serde_json::{Value, json};
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

pub struct Loaded {
    config: Config,
    selected: Selection,
    alternative: Option<Selection>,
}

#[derive(Clone)]
struct Selection {
    root: PathBuf,
    source: Source,
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
    ) -> Result<Loaded> {
        let file = selected(file)?;
        let mut config = Self::default();
        let mut source =
            (!config.domain_space_root.as_os_str().is_empty()).then_some(Source::Default);
        if let Some(file) = &file {
            let seen = plumb::config::load::<ConfigPartial>(&file.path)
                .map_err(|error| Error::new(error.to_string()))?;
            if seen.domain_space_root.is_some() {
                source = Some(Source::Config(file.clone()));
            }
            config = config.merge(seen);
        }
        let environment = <Self as Cascade>::env(&Self::prefix())
            .map_err(|error| Error::new(error.to_string()))?;
        if environment.domain_space_root.is_some() {
            source = Some(Source::Environment);
        }
        config = config.merge(environment);
        let configured = source.clone().map(|source| Selection {
            root: config.domain_space_root.clone(),
            source,
        });
        if root.is_some() {
            source = Some(Source::Explicit);
        }
        config = config.merge(ConfigPartial {
            domain_space_root: root.map(Path::to_path_buf),
            home: home.map(Path::to_path_buf),
            releases: releases.map(str::to_string),
            depot: None,
        });
        let config = config.validate()?;
        let selected = Selection {
            root: config.domain_space_root.clone(),
            source: source.unwrap_or(Source::Default),
        };
        let alternative = matches!(selected.source, Source::Explicit)
            .then_some(configured)
            .flatten()
            .filter(|alternative| alternative.root != selected.root);
        Ok(Loaded {
            config,
            selected,
            alternative,
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

    pub fn root(&self) -> Result<Root> {
        if self.selected.root.as_os_str().is_empty() {
            return Err(Error::detailed(
                "concord.space.root",
                "no Space root is configured; inspect `concord config show`, then set --root, CONCORD_DOMAIN_SPACE_ROOT, or domain_space_root",
                json!({"root": "", "provenance": self.selected.source.json()}),
            ));
        }
        Root::new(&self.selected.root)
    }

    pub fn absent(&self, root: &Root, error: Error) -> Error {
        if error.code() != "concord.estate.absent" {
            return error;
        }
        let estate = root.path().join(".concord");
        let alternative = self.alternative.as_ref().map(Selection::json);
        Error::detailed(
            "concord.estate.absent",
            self.selected
                .message(root.path(), &estate, alternative.as_ref()),
            json!({
                "root": root.path().display().to_string(),
                "estate": estate.display().to_string(),
                "provenance": self.selected.source.json(),
                "configured_alternative": alternative,
            }),
        )
    }
}

impl Selection {
    fn json(&self) -> Value {
        json!({
            "root": resolved(&self.root).display().to_string(),
            "provenance": self.source.json(),
        })
    }

    fn message(&self, root: &Path, estate: &Path, alternative: Option<&Value>) -> String {
        match (&self.source, alternative) {
            (Source::Explicit, Some(alternative)) => format!(
                "Concord estate is absent at {}; --root selected Space {} while the configured alternative is {}; remove or correct --root, then inspect `concord config show`",
                estate.display(),
                root.display(),
                alternative["root"].as_str().unwrap_or("unavailable"),
            ),
            (Source::Explicit, None) => format!(
                "Concord estate is absent at {}; --root selected Space {}; correct --root or inspect `concord config show`, and bootstrap only if this is intentionally a new Space",
                estate.display(),
                root.display(),
            ),
            (Source::Environment, _) => format!(
                "Concord estate is absent at {}; CONCORD_DOMAIN_SPACE_ROOT selected Space {}; correct or unset it, then inspect `concord config show`",
                estate.display(),
                root.display(),
            ),
            (Source::Config(file), _) => format!(
                "Concord estate is absent at {}; config {} selected Space {}; inspect `concord config show`, and bootstrap only if this is intentionally a new Space",
                estate.display(),
                file.path.display(),
                root.display(),
            ),
            (Source::Default, _) => format!(
                "Concord estate is absent at {}; the built-in default selected Space {}; inspect `concord config show`, and bootstrap only if this is intentionally a new Space",
                estate.display(),
                root.display(),
            ),
        }
    }
}

impl Source {
    fn json(&self) -> Value {
        match self {
            Self::Explicit => json!({"kind": "explicit", "input": "--root"}),
            Self::Environment => json!({
                "kind": "environment",
                "input": "CONCORD_DOMAIN_SPACE_ROOT",
            }),
            Self::Config(file) => json!({
                "kind": "config",
                "input": if file.explicit { "--config" } else { "platform-default" },
                "path": file.path.display().to_string(),
            }),
            Self::Default => json!({"kind": "built-in-default"}),
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

fn resolved(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
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
