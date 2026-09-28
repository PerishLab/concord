use super::super::Estate;
use super::super::artifact::preflight;
use super::issue_stale;
use crate::path::at;
use crate::{Error, Result, component};
use serde::Serialize;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct IssueArtifact {
    pub issue: super::super::super::Coordinate,
    pub node: String,
    pub name: String,
    pub path: PathBuf,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IssueImport {
    pub issue: super::super::super::Coordinate,
    pub name: String,
    pub source: PathBuf,
    pub revision: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IssueRemoval {
    pub issue: super::super::super::Coordinate,
    pub name: String,
    pub revision: i64,
}

impl Estate {
    pub async fn issue_artifacts(
        &self,
        issue: &super::super::super::Coordinate,
    ) -> Result<Vec<IssueArtifact>> {
        let anchor = self.issue(issue).await?;
        let root = self.issue_artifact_root(&anchor)?;
        if !root.exists() {
            return Ok(Vec::new());
        }
        if !direct(&root) {
            return Err(foreign(&root));
        }
        let mut found = Vec::new();
        for entry in std::fs::read_dir(root)? {
            let entry = entry?;
            if entry.file_type()?.is_symlink() || !entry.file_type()?.is_dir() {
                return Err(foreign(&entry.path()));
            }
            let name = entry
                .file_name()
                .into_string()
                .map_err(|_| Error::typed("concord.artifact.name", "Artifact name is not UTF-8"))?;
            component("artifact name", &name)?;
            found.push(IssueArtifact {
                issue: anchor.coordinate.clone(),
                node: anchor.node.clone(),
                name,
                path: entry.path(),
            });
        }
        found.sort_by_key(|artifact| artifact.name.clone());
        Ok(found)
    }

    pub async fn issue_artifact(
        &self,
        issue: &super::super::super::Coordinate,
        name: &str,
    ) -> Result<IssueArtifact> {
        component("artifact name", name)?;
        self.issue_artifacts(issue)
            .await?
            .into_iter()
            .find(|artifact| artifact.name == name)
            .ok_or_else(|| {
                Error::typed(
                    "concord.artifact.absent",
                    format!("Artifact does not exist: {}/{}", issue.identity(), name),
                )
            })
    }

    pub async fn preflight_issue(&self, request: &IssueImport) -> Result<preflight::Survey> {
        component("artifact name", &request.name)?;
        let _guard = self.guard()?;
        self.ensure().await?;
        let anchor = self.issue(&request.issue).await?;
        issue_stale(anchor.revision, request.revision)?;
        let target = self.issue_artifact_root(&anchor)?.join(&request.name);
        vacant(&target)?;
        preflight::inspect(&request.source, &target, &self.space)
    }

    pub async fn import_issue(&self, request: &IssueImport) -> Result<IssueArtifact> {
        component("artifact name", &request.name)?;
        let _guard = self.guard()?;
        self.ensure().await?;
        let anchor = self.issue(&request.issue).await?;
        issue_stale(anchor.revision, request.revision)?;
        let root = self.issue_artifact_root(&anchor)?;
        let target = root.join(&request.name);
        vacant(&target)?;
        let checked = preflight::inspect(&request.source, &target, &self.space)?;
        let source = PathBuf::from(checked.source);
        at(&root).directory()?;
        at(&target).directory()?;
        let copied = if source.is_dir() {
            super::super::artifact::copy::tree(&source, &target)
        } else {
            let name = source
                .file_name()
                .ok_or_else(|| Error::new("Artifact source has no filename"))?;
            super::super::artifact::copy::file(&source, &target.join(name))
        };
        if let Err(error) = copied {
            let _ = std::fs::remove_dir_all(&target);
            prune(&root);
            return Err(error);
        }
        let revision = (anchor.revision + 1).to_string();
        if let Err(error) = self
            .core
            .set("Anchor", anchor.key, &[("revision", revision.as_str())])
            .await
        {
            let _ = std::fs::remove_dir_all(&target);
            prune(&root);
            return Err(super::super::fault(error));
        }
        Ok(IssueArtifact {
            issue: anchor.coordinate,
            node: anchor.node,
            name: request.name.clone(),
            path: target,
        })
    }

    pub async fn remove_issue(&self, request: &IssueRemoval) -> Result<i64> {
        component("artifact name", &request.name)?;
        let _guard = self.guard()?;
        self.ensure().await?;
        let anchor = self.issue(&request.issue).await?;
        issue_stale(anchor.revision, request.revision)?;
        let root = self.issue_artifact_root(&anchor)?;
        let target = root.join(&request.name);
        if !direct(&target) {
            return Err(Error::typed(
                "concord.artifact.absent",
                format!(
                    "Artifact does not exist as a direct directory: {}",
                    target.display()
                ),
            ));
        }
        std::fs::remove_dir_all(&target)?;
        prune(&root);
        let revision = anchor.revision + 1;
        let next = revision.to_string();
        self.core
            .set("Anchor", anchor.key, &[("revision", next.as_str())])
            .await
            .map_err(super::super::fault)?;
        Ok(revision)
    }

    pub(super) fn issue_artifact_root(
        &self,
        anchor: &super::super::super::Anchor,
    ) -> Result<PathBuf> {
        component("issue node", &anchor.node)?;
        Ok(self
            .space
            .join(".issues")
            .join(&anchor.node)
            .join("artifacts"))
    }
}

fn vacant(target: &Path) -> Result<()> {
    if target.exists() || std::fs::symlink_metadata(target).is_ok() {
        return Err(Error::typed(
            "concord.artifact.reserved",
            format!("Artifact seat already exists: {}", target.display()),
        ));
    }
    Ok(())
}

fn direct(path: &Path) -> bool {
    path.is_dir()
        && std::fs::symlink_metadata(path)
            .map(|metadata| !metadata.file_type().is_symlink())
            .unwrap_or(false)
}

fn foreign(path: &Path) -> Error {
    Error::typed(
        "concord.artifact.territory",
        format!(
            "Artifact seat contains foreign territory: {}",
            path.display()
        ),
    )
}

fn prune(root: &Path) {
    let Some(issue) = root.parent() else {
        return;
    };
    for path in [root, issue] {
        let empty = path
            .read_dir()
            .ok()
            .and_then(|mut entries| entries.next())
            .is_none();
        if path.is_dir() && empty {
            let _ = std::fs::remove_dir(path);
        }
    }
}
