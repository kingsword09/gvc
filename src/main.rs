mod agents;
mod cli;
mod error;
mod gradle;
mod maven;
mod repository;
mod utils;
mod workflow;

use clap::Parser;
use cli::{Cli, Commands};
use colored::Colorize;
use std::process;

fn main() {
    let cli = Cli::parse();

    if cli.verbose {
        unsafe {
            std::env::set_var("GVC_VERBOSE", "1");
        }
    }

    let result = match cli.command {
        Commands::Update {
            interactive,
            filter,
            stable_only,
            no_git,
        } => workflow::execute_update(&cli.path, interactive, filter, stable_only, no_git),
        Commands::Check { include_unstable } => {
            workflow::execute_check(&cli.path, !include_unstable)
        }
        Commands::List => workflow::execute_list(&cli.path),
        Commands::Add {
            plugin,
            library,
            coordinate,
            alias,
            version_alias,
            stable_only,
        } => workflow::execute_add(
            &cli.path,
            plugin,
            library,
            &coordinate,
            alias.as_deref(),
            version_alias.as_deref(),
            stable_only,
        ),
    };

    if let Err(e) = result {
        eprintln!("{} {}", "Error:".red().bold(), e);
        process::exit(1);
    }
}
