use super::process;
use plumb::delivery::native::{Context, Evidence, Gate, Identity};
use plumb::landing::Refusal;
use radicula_process::{Search, Tool};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::path::Path;
use std::process::Command;

pub(super) const AUTHORITY: &str = concat!("concord.wharf-native/v1@", env!("CONCORD_WHARF_GATE"));
const CHECK: &[&str] = &["-B", "-m", "scripts.selfcheck"];
const TEST: &[&str] = &[
    "-B", "-m", "unittest", "discover", "-s", "tests", "-t", ".", "-q",
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
        let python = tool(search, "python3")?;
        let git = tool(search, "git")?;
        let directories = [
            python.path.parent(),
            git.path.parent(),
            Some(Path::new("/usr/bin")),
            Some(Path::new("/bin")),
        ];
        let paths = directories.into_iter().flatten().collect::<Vec<_>>();
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
        Ok(world)
    }

    fn command(&self, root: &Path, args: &[&str]) -> Command {
        let mut command = Command::new(&self.python.path);
        command
            .current_dir(root)
            .env_clear()
            .env("PATH", &self.path)
            .args(args);
        command
    }
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
