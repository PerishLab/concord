use super::process;
use plumb::delivery::native::{Context, Evidence, Gate};
use plumb::landing::Refusal;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
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

#[derive(Eq, PartialEq, Serialize)]
struct Tool {
    path: PathBuf,
    digest: String,
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
    fn verify(&self, context: &Context<'_>) -> Result<Evidence, Refusal> {
        let world = World::read(context.root)?;
        process::run(&mut world.command(context.root, CHECK))?;
        process::run(&mut world.command(context.root, TEST))?;
        if world != World::read(context.root)? {
            return Err(process::refuse(
                "native gate tool world changed during verification",
            ));
        }
        let input = Receipt {
            authority: AUTHORITY,
            source: context.source,
            target: context.target,
            tree: context.tree,
            check: CHECK,
            test: TEST,
            world,
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
}

impl World {
    fn read(root: &Path) -> Result<Self, Refusal> {
        let python = Tool::read("python3")?;
        let git = Tool::read("git")?;
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

impl Tool {
    fn read(name: &str) -> Result<Self, Refusal> {
        let path = which::which(name)
            .and_then(|path| {
                path.canonicalize()
                    .map_err(|_| which::Error::CannotCanonicalize)
            })
            .map_err(|error| process::refuse(format!("native gate requires {name}: {error}")))?;
        let digest = process::hash(&path)?;
        Ok(Self { path, digest })
    }
}
