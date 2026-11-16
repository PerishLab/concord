use super::super::{Relocate, Seat, entry};
use concord_core::{Admission, Coordinate, Register, Repository, Start};
use std::path::{Path, PathBuf};

pub(super) struct Fixture {
    pub temp: tempfile::TempDir,
    pub task: Relocate,
    pub repository: PathBuf,
}

impl Fixture {
    pub async fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("legacy");
        let home = temp.path().join("home");
        let repository = temp.path().join("repository");
        for path in [&source, &home, &repository] {
            std::fs::create_dir(path).unwrap();
        }
        git(&repository, &["init", "-b", "main"]);
        git(&repository, &["config", "user.name", "Relocate Test"]);
        git(
            &repository,
            &["config", "user.email", "relocate@example.invalid"],
        );
        git(&repository, &["config", "commit.gpgsign", "false"]);
        git(&repository, &["config", "core.hooksPath", "/dev/null"]);
        std::fs::write(repository.join("source"), "committed source").unwrap();
        git(&repository, &["add", "source"]);
        git(&repository, &["commit", "-m", "source"]);
        let remote = temp.path().join("remote.git");
        git(&repository, &["init", "--bare", remote.to_str().unwrap()]);
        git(
            &repository,
            &[
                "remote",
                "add",
                "origin",
                "https://github.com/PerishLab/concord.git",
            ],
        );
        git(
            &repository,
            &[
                "config",
                &format!("url.{}.insteadOf", remote.display()),
                "https://github.com/PerishLab/concord.git",
            ],
        );
        git(&repository, &["push", "-u", "origin", "main"]);
        let seat = Seat::new(temp.path().join("seed"));
        let estate = seat.bootstrap().await.unwrap();
        estate
            .register(&Register {
                node: "R_concord".into(),
                repository: Repository::parse("PerishLab/concord").unwrap(),
                path: repository.clone(),
            })
            .await
            .unwrap();
        estate
            .admit(&Admission {
                node: "I_retained".into(),
                coordinate: Coordinate::parse("PerishLab/concord#466").unwrap(),
            })
            .await
            .unwrap();
        estate
            .start(&Start {
                issue: Coordinate::parse("PerishLab/concord#466").unwrap(),
                node: "I_retained".into(),
                stable: "R_concord".into(),
                kind: "Task".into(),
                claims: vec!["source".into()],
                revision: 0,
            })
            .await
            .unwrap();
        drop(estate);
        std::fs::create_dir(source.join(".concord")).unwrap();
        std::fs::copy(seat.database(), source.join(".concord/estate.sqlite3")).unwrap();
        std::fs::copy(seat.sudo(), source.join(".concord/sudo")).unwrap();
        let parent = source.join(".issues/I_retained");
        std::fs::create_dir_all(&parent).unwrap();
        let seed = temp
            .path()
            .join("seed/workspace/issues/I_retained/worktree");
        let worktree = parent.join("worktree");
        git(
            &repository,
            &[
                "worktree",
                "move",
                seed.to_str().unwrap(),
                worktree.to_str().unwrap(),
            ],
        );
        std::fs::write(worktree.join("staged"), "index").unwrap();
        std::fs::write(worktree.join(".gitignore"), "ignored\n").unwrap();
        git(&worktree, &["add", "staged", ".gitignore"]);
        std::fs::write(worktree.join("staged"), "dirty").unwrap();
        std::fs::write(worktree.join("untracked"), "untracked").unwrap();
        std::fs::write(worktree.join("ignored"), "ignored").unwrap();
        std::os::unix::fs::symlink("staged", worktree.join("link")).unwrap();
        let artifacts = parent.join("artifacts");
        std::fs::create_dir_all(artifacts.join("evidence")).unwrap();
        std::fs::write(artifacts.join("evidence/source"), "private evidence").unwrap();
        Self {
            temp,
            task: Relocate { source, home },
            repository,
        }
    }

    pub async fn run(&self, command: &str) -> concord_core::Result<()> {
        entry(vec![
            command.into(),
            self.task.source.clone().into(),
            self.task.home.clone().into(),
            "--stopped".into(),
        ])
        .await
    }

    pub fn old(&self) -> PathBuf {
        self.task.source.join(".issues/I_retained/worktree")
    }
    pub fn target(&self) -> PathBuf {
        self.task.home.join("workspace/issues/I_retained/worktree")
    }
}

pub(super) fn git(root: &Path, args: &[&str]) -> String {
    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(root)
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .env_remove("GIT_INDEX_FILE")
        .args([
            "--no-optional-locks",
            "-c",
            "core.fsmonitor=false",
            "-c",
            "core.hooksPath=/dev/null",
        ])
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap().trim().into()
}
