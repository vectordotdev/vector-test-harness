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
    /// Verify a Vector correctness case or finite workload locally.
    Check(CheckArgs),
    /// Measure the finite producer-to-verifier path without Vector.
    Calibrate(CalibrateArgs),
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
    #[value(name = "file_to_tcp_performance")]
    FileToTcp,
    #[value(name = "tcp_to_tcp_performance")]
    TcpToTcp,
    #[value(name = "regex_parsing_performance")]
    RegexParsing,
    #[value(name = "tcp_to_http_performance")]
    TcpToHttp,
    #[value(name = "tcp_to_blackhole_performance")]
    TcpToBlackhole,
}

impl CheckCase {
    pub fn name(self) -> &'static str {
        match self {
            Self::WrappedJson => "wrapped_json_correctness",
            Self::Sighup => "sighup_correctness",
            Self::FileToTcp => "file_to_tcp_performance",
            Self::TcpToTcp => "tcp_to_tcp_performance",
            Self::RegexParsing => "regex_parsing_performance",
            Self::TcpToHttp => "tcp_to_http_performance",
            Self::TcpToBlackhole => "tcp_to_blackhole_performance",
        }
    }

    pub fn is_workload(self) -> bool {
        !matches!(self, Self::WrappedJson | Self::Sighup)
    }
}

#[derive(Debug, Args)]
pub struct CheckArgs {
    #[arg(value_enum)]
    pub case: CheckCase,
    #[arg(long)]
    pub vector_bin: PathBuf,
    /// Linux CPU list for Vector only, e.g. 0,2-3 (requires taskset).
    #[arg(long, value_parser = cpu_list)]
    pub vector_cpus: Option<String>,
    /// Case budget, including startup, delivery, reload, and graceful shutdown.
    #[arg(long, default_value_t = 30, value_parser = clap::value_parser!(u64).range(1..=3600))]
    pub timeout_secs: u64,
    /// New artifact directory. Existing directories are never overwritten.
    #[arg(long)]
    pub output: Option<PathBuf>,
    #[command(flatten)]
    pub delivery: DeliveryArgs,
}

fn cpu_list(value: &str) -> Result<String, String> {
    parse_cpu_list(value).map(|_| value.to_owned())
}

pub(crate) fn parse_cpu_list(value: &str) -> Result<std::collections::BTreeSet<u16>, String> {
    let invalid = || "expected a nonempty CPU list such as 0,2-3 (CPU IDs 0..65535)".to_owned();
    let mut cpus = std::collections::BTreeSet::new();
    for part in value.split(',') {
        let parse = |text: &str| {
            if text.is_empty() || !text.bytes().all(|byte| byte.is_ascii_digit()) {
                return Err(invalid());
            }
            text.parse::<u16>().map_err(|_| invalid())
        };
        if let Some((start, end)) = part.split_once('-') {
            let start = parse(start)?;
            let end = parse(end)?;
            if start > end {
                return Err(invalid());
            }
            cpus.extend(start..=end);
        } else {
            cpus.insert(parse(part)?);
        }
    }
    Ok(cpus)
}

#[derive(Debug, Clone, Args, serde::Serialize)]
pub struct DeliveryArgs {
    /// Measured records in a finite workload (in addition to canary and warmup).
    #[arg(long, default_value_t = 10_000, value_parser = clap::value_parser!(u32).range(1..=1_000_000))]
    pub events: u32,
    /// Maximum producer write size; changes IO batching, not logical records.
    #[arg(long, default_value_t = 16_384, value_parser = clap::value_parser!(u32).range(1024..=1_048_576))]
    pub producer_chunk_bytes: u32,
    /// Maximum TCP receiver reads per pass; HTTP uses its own async body reader.
    #[arg(long, default_value_t = 16, value_parser = clap::value_parser!(u16).range(1..=256))]
    pub receiver_reads_per_pass: u16,
}

impl DeliveryArgs {
    pub fn is_default(&self) -> bool {
        self.events == 10_000
            && self.producer_chunk_bytes == 16_384
            && self.receiver_reads_per_pass == 16
    }
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum CalibrationTransport {
    Tcp,
    ParsedJson,
    Http,
}

#[derive(Debug, Args)]
pub struct CalibrateArgs {
    #[arg(value_enum)]
    pub transport: CalibrationTransport,
    #[command(flatten)]
    pub delivery: DeliveryArgs,
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
