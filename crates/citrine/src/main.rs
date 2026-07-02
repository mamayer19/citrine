mod config;
mod library;
mod plumbing;
mod settings;
mod tui;

use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(name = "citrine", version, about)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand, Debug)]
enum Command {
    Tui,
    #[command(about = "Write a theme config for a format")]
    Export(plumbing::ExportArgs),
    #[command(about = "Query the hosting terminal colors and compare them to an expected palette")]
    Probe(plumbing::ProbeArgs),
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    match Cli::parse().command {
        Some(Command::Tui) | None => tui::run(),
        Some(Command::Export(args)) => plumbing::run_export(args),
        Some(Command::Probe(args)) => std::process::exit(plumbing::run_probe(args)?),
    }
}
