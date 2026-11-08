use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

const SCHEMA: &str = "concord.space/v1";

#[derive(Clone, Debug)]
pub(super) struct Layout {
    pub(super) root: PathBuf,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Identity {
    schema: String,
    id: String,
}

impl Layout {
    pub(super) fn new(home: &Path) -> Self {
        Self {
            root: home.join("workspace"),
        }
    }

    pub(super) fn resolved(&self) -> Result<Self> {
        Ok(Self {
            root: self.root.canonicalize()?,
        })
    }

    pub(super) fn database(&self) -> PathBuf {
        self.root.join("estate.sqlite3")
    }

    pub(super) fn sudo(&self) -> PathBuf {
        self.root.join("sudo")
    }

    pub(super) fn locks(&self) -> PathBuf {
        self.root.join("locks")
    }

    pub(super) fn issue(&self, node: &str) -> PathBuf {
        self.root.join("issues").join(node)
    }

    pub(super) fn publish(&self) -> Result<String> {
        let mut bytes = [0u8; 32];
        getrandom::fill(&mut bytes).map_err(|error| fault(error.to_string()))?;
        let id = bytes
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        let identity = Identity {
            schema: SCHEMA.into(),
            id: id.clone(),
        };
        let text = serde_json::to_string(&identity).map_err(|error| fault(error.to_string()))?;
        crate::path::at(&self.root.join("identity.json")).file(&text)?;
        Ok(id)
    }

    pub(super) fn read(&self) -> Result<String> {
        direct(&self.root, true)?;
        direct(&self.locks(), true)?;
        direct(&self.database(), false)?;
        direct(&self.sudo(), false)?;
        for name in ["issues", "activity", "occupancy"] {
            let path = self.root.join(name);
            if path.symlink_metadata().is_ok() {
                direct(&path, true)?;
            }
        }
        let path = self.root.join("identity.json");
        direct(&path, false)?;
        let identity: Identity = serde_json::from_slice(&std::fs::read(path)?)
            .map_err(|error| fault(format!("invalid Space identity: {error}")))?;
        if identity.schema != SCHEMA
            || identity.id.len() != 64
            || !identity
                .id
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(fault(
                "Space identity or layout version is invalid; preserve and inspect the data home",
            ));
        }
        Ok(identity.id)
    }
}

pub(super) fn direct(path: &Path, directory: bool) -> Result<()> {
    let metadata = std::fs::symlink_metadata(path)
        .map_err(|error| fault(format!("cannot inspect {}: {error}", path.display())))?;
    #[cfg(windows)]
    let redirected = {
        use std::os::windows::fs::MetadataExt;
        metadata.file_attributes() & 0x400 != 0
    };
    #[cfg(not(windows))]
    let redirected = metadata.file_type().is_symlink();
    if redirected
        || if directory {
            !metadata.is_dir()
        } else {
            !metadata.is_file()
        }
    {
        return Err(fault(format!(
            "{} must be a direct {}",
            path.display(),
            if directory {
                "directory"
            } else {
                "regular file"
            }
        )));
    }
    Ok(())
}

fn fault(message: impl Into<String>) -> Error {
    Error::typed("concord.space.identity", message)
}
