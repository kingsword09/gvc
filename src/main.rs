mod agents;
mod cli;
mod error;
mod gradle;
mod maven;
mod repository;
mod utils;
mod workflow;

use clap::Parser;
use cli::{Cli, Commands, OutputFormat};
use colored::Colorize;
use std::process;
use utils::{output, verbose};
use workflow::{AddOptions, RunOptions, UpdateOptions, WorkflowStatus};

fn main() {
    let cli = Cli::parse();
    if cli.no_color || cli.format == OutputFormat::Json {
        colored::control::set_override(false);
    }
    output::init(cli.quiet || cli.format == OutputFormat::Json);
    verbose::init(cli.verbose);

    let options = RunOptions {
        catalog_path: cli.catalog.as_deref(),
        output_format: cli.format,
    };

    let result = match cli.command {
        Commands::Update {
            interactive,
            filter,
            target,
            stable_only: _,
            no_stable_only,
            no_git,
            dry_run,
            apply,
        } => workflow::execute_update(
            &cli.path,
            options,
            UpdateOptions {
                interactive,
                target_filter: target.or(filter),
                stable_only: !no_stable_only,
                no_git,
                dry_run,
                explicit_apply: apply,
            },
        ),
        Commands::Check {
            include_unstable,
            fail_on_updates,
        } => workflow::execute_check(&cli.path, options, !include_unstable, fail_on_updates),
        Commands::Outdated {
            include_unstable,
            fail_on_updates,
        } => workflow::execute_outdated(&cli.path, options, !include_unstable, fail_on_updates),
        Commands::List => workflow::execute_list(&cli.path, options),
        Commands::Audit { fail_on_issues } => {
            workflow::execute_audit(&cli.path, options, fail_on_issues)
        }
        Commands::Doctor { fail_on_issues } => {
            workflow::execute_doctor(&cli.path, options, fail_on_issues)
        }
        Commands::Add {
            plugin,
            library: _,
            coordinate,
            alias,
            version_alias,
            stable_only,
            update_version_alias,
        } => workflow::execute_add(
            &cli.path,
            options,
            AddOptions {
                plugin_flag: plugin,
                coordinate: &coordinate,
                alias_override: alias.as_deref(),
                version_alias_override: version_alias.as_deref(),
                stable_only,
                allow_version_alias_update: update_version_alias,
            },
        ),
    };

    match result {
        Ok(status) => process::exit(status.exit_code()),
        Err(e) => {
            if cli.format == OutputFormat::Json {
                let error = serde_json::json!({
                    "status": "error",
                    "error": {
                        "kind": e.kind(),
                        "message": e.to_string(),
                    }
                });
                println!("{}", serde_json::to_string_pretty(&error).unwrap_or_else(|_| {
                    "{\"status\":\"error\",\"error\":{\"kind\":\"serialization\",\"message\":\"failed to render error\"}}".to_string()
                }));
            } else {
                eprintln!("{} {}", "Error:".red().bold(), e);
            }
            process::exit(WorkflowStatus::Success.exit_code() + 1);
        }
    }
}
