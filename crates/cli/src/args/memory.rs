use clap::{Args, Subcommand};
use std::path::PathBuf;

#[derive(Args)]
pub struct MemoryArgs {
    #[command(subcommand)]
    pub command: MemoryCommand,
}

#[derive(Subcommand)]
pub enum MemoryCommand {
    #[command(about = "Create .task/ and its initial MAIN.md")]
    Init {
        task: String,
        #[arg(
            long,
            value_name = "PATH|-",
            help = "Read MAIN.md from PATH or stdin (-); files are consumed by default"
        )]
        file: PathBuf,
        #[arg(long, help = "Retain a file input after successful initialization")]
        keep_file: bool,
        #[arg(long)]
        dry_run: bool,
    },
    #[command(about = "Read MAIN.md with its content revision")]
    Read {
        task: String,
        #[arg(
            long,
            value_name = "KEY",
            help = "Project a fixed v1 section as a directly patchable sparse document"
        )]
        section: Vec<String>,
    },
    #[command(about = "Apply a sparse concord-memory:v1 section patch")]
    Patch {
        task: String,
        #[arg(long, help = "Must match the revision embedded in the patch when set")]
        expect: Option<String>,
        #[arg(
            long,
            value_name = "PATH|-",
            help = "Read the sparse patch from PATH or stdin (-); files are consumed by default"
        )]
        file: PathBuf,
        #[arg(long, help = "Retain a file input after a successful mutation")]
        keep_file: bool,
        #[arg(long)]
        dry_run: bool,
    },
    #[command(about = "Replace MAIN.md at an expected revision")]
    Write {
        task: String,
        #[arg(long)]
        expect: String,
        #[arg(
            long,
            value_name = "PATH|-",
            help = "Read MAIN.md from PATH or stdin (-); files are consumed by default"
        )]
        file: PathBuf,
        #[arg(long, help = "Retain a file input after a successful mutation")]
        keep_file: bool,
        #[arg(long)]
        dry_run: bool,
    },
    #[command(about = "Allocate a phase and replace MAIN.md")]
    Settle {
        task: String,
        #[arg(long)]
        expect: String,
        #[arg(
            long,
            value_name = "PATH|-",
            help = "Read the phase from PATH or stdin (-); files are consumed by default"
        )]
        phase_file: PathBuf,
        #[arg(
            long,
            value_name = "PATH|-",
            help = "Read MAIN.md from PATH or stdin (-); files are consumed by default"
        )]
        main_file: PathBuf,
        #[arg(long, help = "Retain both file inputs after a successful settle")]
        keep_files: bool,
        #[arg(long)]
        dry_run: bool,
    },
    #[command(about = "List or read immutable memory phases")]
    Phase {
        #[command(subcommand)]
        command: MemoryPhaseCommand,
    },
    #[command(about = "Remove the exact .task/ tree")]
    Remove {
        task: String,
        #[arg(long)]
        apply: bool,
    },
}

#[derive(Subcommand)]
pub enum MemoryPhaseCommand {
    #[command(about = "List validated immutable phases")]
    List { task: String },
    #[command(about = "Read one validated immutable phase")]
    Read { task: String, number: u32 },
}
