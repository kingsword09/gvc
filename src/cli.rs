use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(
    name = "gvc",
    about = "Gradle Version Catalog - A tool to manage Gradle dependency updates",
    version,
    author
)]
pub struct Cli {
    /// Path to the project directory (defaults to current directory)
    #[arg(short, long, default_value = ".")]
    pub path: String,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Update dependencies in the version catalog
    Update {
        /// Enable interactive mode to review updates before applying
        #[arg(short, long)]
        interactive: bool,

        /// Only update to stable versions (no alpha, beta, RC)
        #[arg(short, long)]
        stable_only: bool,

        /// Skip Git operations (don't create branch or commit)
        #[arg(long)]
        no_git: bool,
    },

    /// Check for available dependency updates without applying them
    Check {
        /// Include unstable versions (alpha, beta, RC)
        #[arg(long)]
        include_unstable: bool,
    },

    /// List all dependencies in the version catalog
    List,
}
