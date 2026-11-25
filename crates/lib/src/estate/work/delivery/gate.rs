use super::process;
use plumb::delivery::native::{Context, Evidence, Gate, Identity};
use plumb::landing::Refusal;
use radicula_process::{Search, Tool};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::process::Command;

pub(super) const AUTHORITY: &str = concat!("concord.wharf-native/v1@", env!("CONCORD_WHARF_GATE"));
const CHECK: &[&str] = &["-B", "-m", "scripts.selfcheck"];
const TEST: &[&str] = &[
    "-B", "-m", "unittest", "discover", "-s", "tests", "-t", ".", "-q",
];
const WINDOWS: &[&str] = &[
    "SystemRoot",
    "SYSTEMROOT",
    "WINDIR",
    "TEMP",
    "TMP",
    "PATHEXT",
];

pub(super) struct Wharf;

#[derive(Eq, PartialEq, Serialize)]
struct World {
    python: Tool,
    git: Tool,
    path: String,
    version: String,
}

#[derive(Serialize)]
struct Receipt<'a> {
    authority: &'a str,
    source: &'a str,
    target: &'a str,
    tree: &'a str,
    check: &'a [&'a str],
    test: &'a [&'a str],
    world: World,
}

impl Gate for Wharf {
    #[locus::trace(with = plumb::observation::view())]
    fn verify(&self, context: &Context<'_>) -> Result<Evidence, Refusal> {
        let world = World::read(context.root)?;
        process::run(&mut world.command(context.root, CHECK))?;
        process::suite(&mut world.command(context.root, TEST))?;
        if world != World::read(context.root)? {
            return Err(process::refuse(
                "native gate tool world changed during verification",
            ));
        }
        world.evidence(context)
    }
}

impl Identity for Wharf {
    fn identify(&self, context: &Context<'_>) -> Result<Evidence, Refusal> {
        World::read(context.root)?.evidence(context)
    }
}

impl World {
    fn evidence(self, context: &Context<'_>) -> Result<Evidence, Refusal> {
        let input = Receipt {
            authority: AUTHORITY,
            source: context.source,
            target: context.target,
            tree: context.tree,
            check: CHECK,
            test: TEST,
            world: self,
        };
        let bytes =
            serde_json::to_vec(&input).map_err(|error| process::refuse(error.to_string()))?;
        Ok(Evidence {
            authority: AUTHORITY.to_string(),
            source: context.source.to_string(),
            tree: context.tree.to_string(),
            digest: format!("{:x}", Sha256::digest(bytes)),
        })
    }

    fn read(root: &Path) -> Result<Self, Refusal> {
        Self::resolve(root, &crate::config::tools().map_err(process::refuse)?)
    }

    fn resolve(root: &Path, search: &Search) -> Result<Self, Refusal> {
        let python = python(search)?;
        let git = tool(search, "git")?;
        let owned = directories(&python, &git);
        let paths = owned.iter().map(PathBuf::as_path).collect::<Vec<_>>();
        let path = crate::config::search(&paths)
            .map_err(|error| process::refuse(error.to_string()))?
            .into_string()
            .map_err(|_| process::refuse("native PATH is not UTF-8"))?;
        let mut world = Self {
            python,
            git,
            path,
            version: String::new(),
        };
        world.version = process::run(&mut world.command(root, &["--version"]))?;
        if cfg!(windows) && !world.version.trim().starts_with("Python 3.") {
            return Err(process::refuse(format!(
                "native gate requires Python 3, found: {}",
                world.version.trim()
            )));
        }
        Ok(world)
    }

    fn command(&self, root: &Path, args: &[&str]) -> Command {
        let mut command = Command::new(&self.python.path);
        command
            .current_dir(root)
            .env_clear()
            .env("PATH", &self.path)
            .env("PYTHONUTF8", "1")
            .env("PYTHONIOENCODING", "utf-8");
        if cfg!(windows) {
            for name in WINDOWS {
                if let Some(value) = crate::config::variable(name) {
                    command.env(name, value);
                }
            }
        }
        command.args(args);
        command
    }
}

fn python(search: &Search) -> Result<Tool, Refusal> {
    if cfg!(windows) {
        tool(search, "python3").or_else(|_| tool(search, "python"))
    } else {
        tool(search, "python3")
    }
}

fn directories(python: &Tool, git: &Tool) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if let Some(parent) = python.path.parent() {
        paths.push(parent.to_path_buf());
    }
    if let Some(parent) = git.path.parent() {
        paths.push(parent.to_path_buf());
    }
    if cfg!(unix) {
        paths.push(PathBuf::from("/usr/bin"));
        paths.push(PathBuf::from("/bin"));
    }
    if cfg!(windows)
        && let Some(system) =
            crate::config::variable("SystemRoot").or_else(|| crate::config::variable("SYSTEMROOT"))
    {
        paths.push(Path::new(&system).join("System32"));
    }
    paths
}

fn tool(search: &Search, name: &str) -> Result<Tool, Refusal> {
    let path = search
        .resolve(name)
        .map_err(|error| process::refuse(format!("native gate requires {name}: {error}")))?;
    Tool::at(path).map_err(|error| process::refuse(error.to_string()))
}

#[cfg(all(test, unix))]
mod tests {
    use super::{Search, World};

    #[test]
    fn aliases() {
        let seat = tempfile::tempdir().expect("seat");
        let root = seat.path().canonicalize().expect("root");
        let bin = root.join("bin");
        let subject = root.join("subject");
        std::fs::create_dir(&bin).expect("bin");
        std::fs::create_dir(&subject).expect("subject");
        let source = root.join("interpreter");
        plumb::testing::executable(&source, "#!/bin/sh\nprintf '%s\\n' \"$0\"\n");
        for name in ["python3", "git"] {
            std::os::unix::fs::symlink(&source, bin.join(name)).expect("alias");
        }
        let search = Search::new(&root, Some("bin".into()), None).expect("search");
        let world = World::resolve(&subject, &search).expect("native world");
        assert_eq!(world.python.path, bin.join("python3"));
        assert_eq!(world.git.path, bin.join("git"));
        assert_eq!(
            world.version.trim(),
            bin.join("python3").to_str().expect("path")
        );
        assert_eq!(world.python.digest, world.git.digest);
        let value = serde_json::to_value(&world.python).expect("tool projection");
        assert_eq!(value.as_object().expect("tool").len(), 2);
        assert_eq!(value["path"], bin.join("python3").to_str().expect("path"));
        assert_eq!(value["digest"], world.python.digest);
    }
}

#[cfg(all(test, windows))]
mod windows {
    use super::{Search, World, process};

    #[test]
    fn native() {
        let seat = tempfile::tempdir().expect("seat");
        let world = World::read(seat.path()).expect("world");
        assert!(world.version.trim().starts_with("Python 3."));
        assert!(world.python.path.is_file());
        assert!(world.git.path.is_file());
        if crate::config::variable("SystemRoot")
            .or_else(|| crate::config::variable("SYSTEMROOT"))
            .is_some()
        {
            assert!(world.path.to_ascii_lowercase().contains("system32"));
        }
        let mut command = world.command(
            seat.path(),
            &[
                "-B",
                "-c",
                "import os, subprocess; subprocess.run(['git', '--version'], check=True, stdout=subprocess.DEVNULL); print(os.environ['PYTHONUTF8'], os.environ['PYTHONIOENCODING'], os.environ.get('USERPROFILE'), '✓')",
            ],
        );
        assert_eq!(
            process::run(&mut command).expect("utf8").trim(),
            "1 utf-8 None ✓"
        );
    }

    #[test]
    fn fallback() {
        let seat = tempfile::tempdir().expect("seat");
        let root = seat.path().canonicalize().expect("root");
        let subject = root.join("subject");
        let bin = root.join("bin");
        let legacy = root.join("legacy");
        std::fs::create_dir(&subject).expect("subject");
        std::fs::create_dir(&bin).expect("bin");
        std::fs::create_dir(&legacy).expect("legacy");
        let host = World::read(&subject).expect("host");
        std::fs::write(
            bin.join("python.cmd"),
            format!("@echo off\r\n\"{}\" %*\r\n", host.python.path.display()),
        )
        .expect("python");
        std::fs::write(bin.join("git.cmd"), "@echo off\r\nexit /b 0\r\n").expect("git");
        let search = Search::new(
            &root,
            Some(bin.clone().into_os_string()),
            Some(".CMD".into()),
        )
        .expect("search");
        let world = World::resolve(&subject, &search).expect("fallback");
        assert_eq!(world.python.path, bin.join("python.cmd"));
        assert!(world.version.trim().starts_with("Python 3."));
        std::fs::write(
            legacy.join("python.cmd"),
            "@echo off\r\necho Python 2.7.18\r\n",
        )
        .expect("legacy");
        std::fs::write(legacy.join("git.cmd"), "@echo off\r\nexit /b 0\r\n").expect("git");
        let search = Search::new(&root, Some(legacy.into_os_string()), Some(".CMD".into()))
            .expect("legacy search");
        let error = World::resolve(&subject, &search).err().expect("python 2");
        assert!(error.message.contains("Python 3"));
    }
}
