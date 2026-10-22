use crate::Result;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[derive(Default)]
pub(crate) struct Identities(BTreeMap<PathBuf, PathBuf>);

impl Identities {
    pub(crate) fn read(&mut self, path: &Path) -> Result<PathBuf> {
        if let Some(identity) = self.0.get(path) {
            return Ok(identity.clone());
        }
        let identity = super::at(path).identity()?;
        self.0.insert(path.to_path_buf(), identity.clone());
        Ok(identity)
    }
}

#[cfg(test)]
mod tests {
    use super::Identities;
    use std::path::{Path, PathBuf};
    use std::process::Command;

    #[test]
    fn snapshot() {
        let temp = tempfile::tempdir().expect("Git identities");
        let first = repository(temp.path(), "first");
        let second = repository(temp.path(), "second");
        let checkout = temp.path().join("checkout");
        std::fs::create_dir(&checkout).expect("checkout");
        point(&checkout, &first);
        let mut identities = Identities::default();
        assert_eq!(identities.read(&checkout).unwrap(), first);
        point(&checkout, &second);
        assert_eq!(identities.read(&checkout).unwrap(), first);
        assert_eq!(Identities::default().read(&checkout).unwrap(), second);
    }

    #[test]
    fn refusal() {
        let temp = tempfile::tempdir().expect("Git identities");
        let first = repository(temp.path(), "first");
        let checkout = temp.path().join("checkout");
        std::fs::create_dir(&checkout).expect("checkout");
        let mut identities = Identities::default();
        assert!(identities.read(&checkout).is_err());
        point(&checkout, &first);
        assert_eq!(identities.read(&checkout).unwrap(), first);
        std::fs::write(checkout.join(".git"), "gitdir: absent\n").unwrap();
        assert_eq!(identities.read(&checkout).unwrap(), first);
        assert!(Identities::default().read(&checkout).is_err());
    }

    #[test]
    fn distinct() {
        let temp = tempfile::tempdir().expect("Git identities");
        let first = repository(temp.path(), "first");
        let second = repository(temp.path(), "second");
        let mut identities = Identities::default();
        assert_eq!(identities.read(&first).unwrap(), first);
        assert_eq!(identities.read(&second).unwrap(), second);
        assert!(identities.read(&temp.path().join("absent")).is_err());
    }

    fn repository(root: &Path, name: &str) -> PathBuf {
        let path = root.join(name);
        assert!(
            Command::new("git")
                .args(["init", "--bare", "--quiet"])
                .arg(&path)
                .status()
                .expect("Git repository")
                .success()
        );
        path.canonicalize().expect("repository identity")
    }

    fn point(checkout: &Path, repository: &Path) {
        std::fs::write(
            checkout.join(".git"),
            format!("gitdir: {}\n", repository.display()),
        )
        .expect("checkout identity");
    }
}
