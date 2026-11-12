use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};

#[derive(Debug, Parser)]
#[command(
    name = "vth",
    version,
    about = "Vector correctness and performance test harness"
)]
pub struct Cli {
    /// Repository root (otherwise discovered from the working directory).
    #[arg(long, global = true)]
    pub root: Option<PathBuf>,
    /// Print planned commands as JSON without executing commands or writing files.
    #[arg(long, global = true)]
    pub dry_run: bool,
    /// Do not read the repository's .env file. Shell .envrc files are never executed.
    #[arg(long, global = true)]
    pub no_dotenv: bool,
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    /// Provision, bootstrap, and run an existing remote case.
    #[command(alias = "test")]
    Run(RunArgs),
    /// Destroy one case/configuration/user's Terraform resources.
    Teardown(TeardownArgs),
    /// Build the benchmark AMI with Packer.
    BuildAmis,
    /// Provision global Athena and results resources with Terraform.
    GlobalBootstrap,
    /// Initialize account-level AWS prerequisites and write .env.
    BootstrapAws(BootstrapArgs),
    /// Inspect archived host metrics; does not validate comparability.
    Compare(CompareArgs),
    /// Summarize dstat CSV results from S3 or local files.
    Results(ResultsArgs),
    /// Export archived metrics in the legacy website format (requires --no-upload).
    Export(ExportArgs),
    /// List available remote and standalone cases.
    List,
    /// Print metadata for scripting.
    Meta {
        #[arg(value_enum)]
        mode: MetaMode,
    },
    /// Check Terraform formatting throughout the repository.
    Lint,
    /// Connect using the same SSH identity as Ansible.
    Ssh {
        #[arg(required = true, trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Report installed external tools without contacting AWS.
    Doctor,
    /// Run an existing correctness case locally with a supplied Vector binary.
    Check(CheckArgs),
    /// Build a pinned revision in an isolated clone, leaving other checkouts alone.
    Build(BuildArgs),
    /// Time a standalone performance workload with supplied binaries.
    Bench(BenchArgs),
    /// Run perf or Valgrind on a standalone workload.
    Profile(ProfileArgs),
    /// Produce flamegraphs and optional differential flamegraphs from perf data.
    Flamegraph(FlamegraphArgs),
    /// Run file_gen for the file_to_blackhole workload.
    Generate(GenerateArgs),
    /// Run the small Rust utilities used on test machines.
    Helper {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Container entrypoint: prepare an optional SSH identity, then run vth.
    #[command(hide = true)]
    Container {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
}

#[derive(Debug, Clone, Args)]
pub struct CaseArgs {
    #[arg(short = 't', long = "test")]
    pub test: Option<String>,
    #[arg(short = 'c', long)]
    pub configuration: Option<String>,
    #[arg(short = 'u', long)]
    pub user_id: Option<String>,
    /// Restrict instance SSH access to this CIDR (otherwise discover the egress IP).
    #[arg(long)]
    pub ssh_cidr: Option<String>,
}

#[derive(Debug, Args)]
pub struct RunArgs {
    #[command(flatten)]
    pub case: CaseArgs,
    #[arg(short = 's', long = "subject", alias = "subjects")]
    pub subject: Option<String>,
    #[arg(short = 'v', long)]
    pub vector_version: Option<String>,
    #[arg(long)]
    pub vector_deb: Option<PathBuf>,
    #[arg(short = 'b', long, conflicts_with = "run_only")]
    pub bootstrap_only: bool,
    #[arg(short = 'r', long)]
    pub run_only: bool,
    #[arg(long)]
    pub skip_terraform: bool,
}

#[derive(Debug, Args)]
pub struct TeardownArgs {
    #[command(flatten)]
    pub case: CaseArgs,
    #[arg(short = 'y', long)]
    pub yes: bool,
}

#[derive(Debug, Args)]
pub struct BootstrapArgs {
    /// Supply an account ID for an offline dry run; checked against AWS on real runs.
    #[arg(long)]
    pub account_id: Option<String>,
    #[arg(long)]
    pub user_id: Option<String>,
    #[arg(long)]
    pub ssh_key: Option<PathBuf>,
    /// Replace an existing .env file.
    #[arg(long)]
    pub force: bool,
}

#[derive(Debug, Clone, Args)]
pub struct CompareArgs {
    #[arg(short = 't', long = "test", required_unless_present = "input")]
    pub test: Option<String>,
    #[arg(short = 'c', long, default_value = "default")]
    pub configuration: String,
    #[arg(short = 's', long)]
    pub subject: Option<String>,
    #[arg(short = 'v', long = "version")]
    pub versions: Vec<String>,
    #[arg(short = 'r', long)]
    pub repair: bool,
    /// Show arithmetic differences from the first column; this does not validate comparability.
    #[arg(short = 'p', long)]
    pub percentages: bool,
    #[arg(long)]
    pub no_percentages: bool,
    /// Read a saved Athena get-query-results response, without AWS.
    #[arg(long)]
    pub input: Option<PathBuf>,
    #[arg(long)]
    pub json: bool,
}

#[derive(Debug, Args)]
pub struct ResultsArgs {
    #[arg(short = 't', long = "test")]
    pub test: String,
    #[arg(short = 'c', long)]
    pub configuration: Option<String>,
    #[arg(short = 'u', long)]
    pub user_id: Option<String>,
    #[arg(long)]
    pub bucket: Option<String>,
    /// Read local dstat CSVs instead of S3.
    #[arg(long)]
    pub input: Vec<PathBuf>,
    #[arg(long)]
    pub json: bool,
}

#[derive(Debug, Args)]
pub struct ExportArgs {
    /// Read a saved Athena response, without querying AWS.
    #[arg(long)]
    pub input: Option<PathBuf>,
    #[arg(long, default_value = "results.json")]
    pub output: PathBuf,
    /// Required: website publication is disabled for unverified comparisons.
    #[arg(long)]
    pub no_upload: bool,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum MetaMode {
    TestCases,
    TestSubjects,
}

#[derive(Debug, Args)]
pub struct BuildArgs {
    #[arg(long, default_value = "https://github.com/vectordotdev/vector.git")]
    pub repository: String,
    #[arg(long)]
    pub revision: String,
    #[arg(long, default_value = "vector")]
    pub binary: String,
    #[arg(long)]
    pub no_default_features: bool,
    #[arg(long)]
    pub features: Option<String>,
    #[arg(long, default_value = "-g", allow_hyphen_values = true)]
    pub rustflags: String,
}

#[derive(Debug, Clone, Args)]
pub struct WorkloadArgs {
    /// Standalone case under cases/performance/.
    pub case: String,
    #[arg(long)]
    pub vector_bin: PathBuf,
    /// Override the case's Vector config.
    #[arg(long)]
    pub config: Option<PathBuf>,
    /// Override the input fixture (plain text or gzip).
    #[arg(long)]
    pub input: Option<PathBuf>,
    #[arg(long)]
    pub output: Option<PathBuf>,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum CheckCase {
    #[value(name = "wrapped_json_correctness")]
    WrappedJson,
    #[value(name = "sighup_correctness")]
    Sighup,
}

impl CheckCase {
    pub fn name(self) -> &'static str {
        match self {
            Self::WrappedJson => "wrapped_json_correctness",
            Self::Sighup => "sighup_correctness",
        }
    }
}

#[derive(Debug, Args)]
pub struct CheckArgs {
    #[arg(value_enum)]
    pub case: CheckCase,
    #[arg(long)]
    pub vector_bin: PathBuf,
    /// Case budget, including startup, delivery, reload, and graceful shutdown.
    #[arg(long, default_value_t = 30, value_parser = clap::value_parser!(u64).range(1..=3600))]
    pub timeout_secs: u64,
    /// New artifact directory. Existing directories are never overwritten.
    #[arg(long)]
    pub output: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct BenchArgs {
    #[command(flatten)]
    pub workload: WorkloadArgs,
    #[arg(long)]
    pub baseline_bin: Option<PathBuf>,
    #[arg(long, default_value_t = 10, value_parser = clap::value_parser!(u32).range(1..))]
    pub runs: u32,
    #[arg(long, default_value_t = 1)]
    pub warmup: u32,
}

#[derive(Debug, Args)]
pub struct ProfileArgs {
    #[command(flatten)]
    pub workload: WorkloadArgs,
    #[arg(long, value_enum, default_value = "perf")]
    pub tool: ProfileTool,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum ProfileTool {
    Perf,
    Stat,
    Massif,
    Dhat,
}

#[derive(Debug, Args)]
pub struct FlamegraphArgs {
    #[arg(long)]
    pub input: PathBuf,
    #[arg(long)]
    pub baseline: Option<PathBuf>,
    #[arg(long, default_value = "out")]
    pub output: PathBuf,
}

#[derive(Debug, Args)]
pub struct GenerateArgs {
    #[arg(long)]
    pub filegen_bin: PathBuf,
    /// Remove existing regular *.log files in this case's logs directory first.
    #[arg(long)]
    pub reset_logs: bool,
}
