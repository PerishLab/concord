use clap::{Args, Subcommand};
use std::path::PathBuf;

pub(crate) const HELP: &str = "Explicitly close one ordinary GitHub Issue against current adopted source/release acceptance. Prepare reads only; apply requires --apply, fresh exact Issue/body/declaration/closure agreement and satisfied acceptance, then requests completed closure once and independently reads back. Human judgment remains explicit. Auto, unknown types and needs labels refuse. No Project, Phase, Member, body or acceptance writes occur. Only github.com is qualified; external gh defaults on PATH and API calls bind github.com. No local estate is opened.

GitHub offers no atomic body/review condition for close: the final read-to-write race remains. A changed or unknown post-write basis is not confirmed success. After any indeterminate result, use read-only recover with the same plan before choosing a fresh explicit preparation/action. Neither observed closed nor observed open proves who acted or authorizes replay. No implicit retries or reopen occur.";

#[derive(Subcommand)]
pub enum Command {
    #[command(about = "Read current satisfied acceptance and prepare one exact closure plan")]
    Prepare(Observe),
    #[command(
        about = "Revalidate and explicitly request one completed closure, then confirm independently"
    )]
    Apply {
        #[command(flatten)]
        read: Read,
        #[arg(long)]
        apply: bool,
    },
    #[command(
        about = "Read current lifecycle and acceptance for a plan without replay or authorship claims"
    )]
    Recover(Read),
}

#[derive(Args)]
pub struct Observe {
    pub issue: String,
    #[arg(long = "github-command", default_value = "gh", value_name = "PROGRAM", help = super::PROGRAM)]
    pub command: PathBuf,
    #[arg(long = "observe-timeout", default_value_t = 10, value_parser = clap::value_parser!(u64).range(1..=60))]
    pub timeout: u64,
    #[arg(long = "max-pages", default_value_t = 20, value_parser = clap::value_parser!(u16).range(1..=20))]
    pub pages: u16,
}

#[derive(Args)]
pub struct Read {
    #[command(flatten)]
    pub observe: Observe,
    #[arg(long, default_value = "-", value_name = "PATH|-")]
    pub input: PathBuf,
}

impl Command {
    pub(crate) fn name(&self) -> &'static str {
        match self {
            Self::Prepare(_) => "issue.close.prepare",
            Self::Apply { .. } => "issue.close.apply",
            Self::Recover(_) => "issue.close.recover",
        }
    }
}
