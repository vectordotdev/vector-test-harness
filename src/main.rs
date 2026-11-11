mod analysis;
mod aws;
mod cli;
mod correctness;
mod guest;
mod infra;
mod local;
mod runtime;

use anyhow::{bail, Result};
use clap::Parser;
use cli::{Cli, Commands};
use runtime::Runtime;

fn main() {
    let cli = Cli::parse();
    if !matches!(cli.command, Commands::Helper { .. }) {
        if let Err(error) = ctrlc::set_handler(|| {
            runtime::INTERRUPTED.store(true, std::sync::atomic::Ordering::SeqCst)
        }) {
            eprintln!("error: could not install interrupt handler: {error}");
            std::process::exit(1);
        }
    }
    if let Err(error) = execute(cli, None) {
        eprintln!("error: {error:#}");
        std::process::exit(1);
    }
}

fn execute(cli: Cli, inherited: Option<Runtime>) -> Result<()> {
    if let Commands::Helper { args } = &cli.command {
        if cli.dry_run {
            bail!("helpers execute on the guest; --dry-run is not supported");
        }
        let code = guest::run(args)?;
        std::process::exit(code);
    }
    if let Commands::Container { args } = &cli.command {
        let args = if args.is_empty() {
            vec!["--help".into()]
        } else {
            args.clone()
        };
        let mut nested = Cli::try_parse_from(std::iter::once("vth".to_owned()).chain(args))
            .unwrap_or_else(|error| error.exit());
        if matches!(nested.command, Commands::Container { .. }) {
            bail!("container cannot invoke itself");
        }
        nested.root = nested.root.or(cli.root);
        nested.dry_run |= cli.dry_run;
        nested.no_dotenv |= cli.no_dotenv;
        let mut runtime = Runtime::new(nested.root.as_deref(), nested.no_dotenv, nested.dry_run)?;
        runtime.container_keys()?;
        return execute(nested, Some(runtime));
    }
    let runtime = match inherited {
        Some(runtime) => runtime,
        None => Runtime::new(cli.root.as_deref(), cli.no_dotenv, cli.dry_run)?,
    };
    match cli.command {
        Commands::Run(args) => infra::run(&runtime, args),
        Commands::Teardown(args) => infra::teardown(&runtime, args),
        Commands::BuildAmis => infra::build_amis(&runtime),
        Commands::GlobalBootstrap => infra::global_bootstrap(&runtime),
        Commands::BootstrapAws(args) => aws::bootstrap(&runtime, args),
        Commands::Compare(args) => analysis::compare(&runtime, args),
        Commands::Results(args) => analysis::results(&runtime, args),
        Commands::Export(args) => analysis::export(&runtime, args),
        Commands::List => infra::meta(&runtime, cli::MetaMode::TestCases),
        Commands::Meta { mode } => infra::meta(&runtime, mode),
        Commands::Lint => infra::lint(&runtime),
        Commands::Ssh { args } => infra::ssh(&runtime, args),
        Commands::Doctor => infra::doctor(&runtime),
        Commands::Check(args) => {
            let code = correctness::run(&runtime, args)?;
            if code != 0 {
                std::process::exit(code);
            }
            Ok(())
        }
        Commands::Build(args) => local::build(&runtime, args),
        Commands::Bench(args) => local::bench(&runtime, args),
        Commands::Profile(args) => local::profile(&runtime, args),
        Commands::Flamegraph(args) => local::flamegraph(&runtime, args),
        Commands::Generate(args) => local::generate(&runtime, args),
        Commands::Container { .. } => unreachable!(),
        Commands::Helper { .. } => unreachable!(),
    }
}
