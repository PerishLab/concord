use clap::Subcommand;
use std::path::PathBuf;

#[derive(clap::Args)]
pub struct Args {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    #[command(
        about = "Read an explicit organization Project without opening local execution state",
        long_about = "Read an organization Project without opening local execution state, selected by OWNER/NUMBER or https://github.com/orgs/OWNER/projects/NUMBER. The result preserves native field ownership, item order, Issue facts and provider cursors. Root connections page within the selected budget; item pages are capped at 20 to keep their nested native projection within GitHub node limits; nested connections are bounded to 100 and retain their continuation cursors. Unknown content, incomplete collections and inaccessible automation details are explicit. This is a live interval, never an atomic snapshot, delegation or write authorization."
    )]
    Show(Observe),
    #[command(about = "List the bounded provider-visible organization Project inventory")]
    List {
        owner: String,
        #[command(flatten)]
        remote: Remote,
    },
    #[command(
        about = "Prepare one qualified native Project change without writes",
        long_about = INTENT
    )]
    Prepare {
        project: String,
        #[arg(long, default_value = "-", value_name = "PATH|-")]
        input: PathBuf,
        #[command(flatten)]
        remote: Remote,
    },
    #[command(about = "Apply one prepared change with current-fact validation and native readback")]
    Apply {
        #[arg(long, default_value = "-", value_name = "PATH|-")]
        input: PathBuf,
        #[arg(long)]
        apply: bool,
        #[command(flatten)]
        remote: Remote,
    },
    #[command(about = "Observe an uncertain change without repeating any provider write")]
    Recover {
        #[arg(long, default_value = "-", value_name = "PATH|-")]
        input: PathBuf,
        #[command(flatten)]
        remote: Remote,
    },
}

#[derive(clap::Args)]
pub struct Observe {
    pub project: String,
    #[arg(long = "github-command")]
    pub command: PathBuf,
    #[arg(long = "page-size", default_value_t = 50, value_parser = clap::value_parser!(u16).range(1..=100))]
    pub size: u16,
    #[arg(long = "max-pages", default_value_t = 20, value_parser = clap::value_parser!(u16).range(1..=100), help = "Maximum pages per root connection")]
    pub pages: u16,
    #[arg(long = "observe-timeout", default_value_t = 10, value_parser = clap::value_parser!(u64).range(1..=60), help = "Timeout in seconds for each provider exchange")]
    pub timeout: u64,
}

#[derive(clap::Args)]
pub struct Remote {
    #[arg(long = "github-command")]
    pub command: PathBuf,
    #[arg(long = "max-pages", default_value_t = 20, value_parser = clap::value_parser!(u16).range(1..=100))]
    pub pages: u16,
    #[arg(long = "observe-timeout", default_value_t = 10, value_parser = clap::value_parser!(u64).range(1..=60))]
    pub timeout: u64,
}

impl Command {
    pub(crate) fn name(&self) -> &'static str {
        match self {
            Self::Show(_) => "project.show",
            Self::List { .. } => "project.list",
            Self::Prepare { .. } => "project.prepare",
            Self::Apply { .. } => "project.apply",
            Self::Recover { .. } => "project.recover",
        }
    }
}

const INTENT: &str = r#"Prepare a native Project change without writes from:
{"reason":"planning rationale","action":{...}}

The target is OWNER/NUMBER or a canonical organization Project URL. Create selects OWNER and establishes a private Project; default workflows must be explicitly qualified before importing candidates. Credentials remain in the external provider command. A prepared plan is an ephemeral operation intent, not delegation or a planning ledger.

Actions (native IDs, never display-name authority):
create: title
configure: changes {title?, description?, readme?, public?}
link: repository (node ID), present (boolean)
candidates: issues (1..500 original Issue node IDs), present (boolean)
field: name, datatype (TEXT/NUMBER/DATE/SINGLE_SELECT/MULTI_SELECT), options
rename: field (Project field node ID), name, options?
remove: field (empty Project-owned field only; cycles/history remain protected)
detach: field (attached Project field node ID), origin (exact organization Issue-field node ID)
view: name, layout (table/board/roadmap), filter, visible, group, vertical, sort
revise: view (node ID), name?, layout?, filter?, visible?
retire: view (node ID)
workflow: workflow (explicit native workflow node ID to remove)
record: body (a new distinct native Project planning update)
planning: change (one of the native planning choices below)

Planning change kinds:
priority: field, item, level (Urgent/High/Medium/Low, or null for unreviewed)
cycle: field (null to create), name, start (YYYY-MM-DD), duration (days), periods
assign: field, item, period (native iteration ID, or null for unassigned)
order: field, period, item, after (same-cycle item ID, or null for cycle beginning)
Each period contains {id:null,title,start,duration}. Updates retain every existing period ID, including completed history; new periods use null. Dates are explicit local calendar dates, never timestamps or elapsed-work evidence. Planning requires Project-owned fields; priority requires exactly four distinct Urgent/High/Medium/Low options. Attached Issue fields, Auto and unknown Issue types remain read-only. Ordering acts only within the selected native cycle and never closes work or changes dependencies.

All actions contain kind, e.g. {"kind":"configure","changes":{"title":"Domain planning"}}.
Select options contain {"id":null,"name":"High","color":"RED","description":""}; updates preserve every existing option ID and cannot import foreign IDs. Colors: GRAY, BLUE, GREEN, YELLOW, ORANGE, RED, PINK, PURPLE. Scalar fields use options: [].
View visible/group/vertical arrays contain qualified native field node IDs; each grouping axis supports one field. Sort contains {"field":"native-node-id","direction":"asc"} or desc. Existing view grouping/sorting cannot be changed through supported provider APIs; explicit new views retain independent identities. Only independently observable field configurations can be written; Type visibility remains a provider projection gap.

Prepare reads current facts twice. Apply requires the complete prepared envelope and --apply, revalidates current facts, retains native requested/confirmed rationale and confirms each step independently. After a successful action mutation acknowledgment, a mismatching readback gets at most three fresh observations with 250 ms between them; provider errors remain immediate unknown outcomes. This never repeats a write. Changed planning context stops confirmation or further writes. An error retains prior confirmations and identifies the unconfirmed step. Recover reads current native state and planning records without repeating any write; observing desired state does not establish authorship. Auto and unknown Issue types remain read-only. Candidate maintenance requires disabled native workflows; external automatic behavior remains unknown. No product worktree or local estate is opened.
"#;
