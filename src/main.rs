mod agents;
mod cli;
mod error;
mod gradle;
mod maven;
mod workflow;

use clap::Parser;
use cli::{Cli, Commands};
use colored::Colorize;
use std::process;

fn main() {
    let cli = Cli::parse();

    let result = match cli.command {
        Commands::Update {
            interactive,
            stable_only,
            no_git,
        } => workflow::execute_update(&cli.path, interactive, stable_only, no_git),
        Commands::Check { include_unstable } => {
            workflow::execute_check(&cli.path, !include_unstable)
        }
        Commands::List => workflow::execute_list(&cli.path),
    };

    if let Err(e) = result {
        eprintln!("{} {}", "Error:".red().bold(), e);
        process::exit(1);
    }
}
