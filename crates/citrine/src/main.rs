mod config;
mod library;
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
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    match Cli::parse().command {
        Some(Command::Tui) | None => tui::run(),
    }
}
