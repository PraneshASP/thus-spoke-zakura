mod runtime;

use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand};
use runtime::{InstanceName, Runtime};

#[derive(Parser)]
#[command(
    name = "thus-spoke-zakura",
    version,
    about = "A one-command Zakura regtest environment"
)]
struct Cli {
    /// Isolated environment name.
    #[arg(long, global = true, default_value = "default")]
    name: InstanceName,
    /// Print machine-readable output where supported.
    #[arg(long, global = true)]
    json: bool,
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Start or resume an environment.
    Start {
        #[arg(long)]
        no_open: bool,
        /// Build project images from the current source before starting.
        #[arg(long)]
        build: bool,
    },
    /// Show service and endpoint status.
    Status,
    /// Open the dashboard in the default browser.
    Open,
    /// Print endpoints for developer tooling.
    Endpoints,
    /// Stream or print service logs.
    Logs {
        #[arg(value_parser = ["app", "zakura", "lightwalletd"])]
        service: Option<String>,
        #[arg(short, long)]
        follow: bool,
    },
    /// Stop an environment without deleting its data.
    Stop,
    /// Delete one environment and all of its volumes.
    Reset {
        #[arg(long)]
        force: bool,
    },
    /// List known environments.
    List,
    /// Check local Docker and configuration prerequisites.
    Doctor,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let runtime = Runtime::discover()?;
    match cli.command.unwrap_or(Command::Start {
        no_open: false,
        build: false,
    }) {
        Command::Start { no_open, build } => runtime.start(&cli.name, no_open, build, cli.json),
        Command::Status => runtime.status(&cli.name, cli.json),
        Command::Open => runtime.open(&cli.name),
        Command::Endpoints => runtime.endpoints(&cli.name, cli.json),
        Command::Logs { service, follow } => runtime.logs(&cli.name, service.as_deref(), follow),
        Command::Stop => runtime.stop(&cli.name),
        Command::Reset { force } => runtime.reset(&cli.name, force),
        Command::List => runtime.list(cli.json),
        Command::Doctor => runtime.doctor(cli.json),
    }
}

fn _assert_pathbuf_send(_: PathBuf) {}
