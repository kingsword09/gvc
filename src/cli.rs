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

    /// Enable verbose output for debugging
    #[arg(short, long, global = true)]
    pub verbose: bool,

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

        /// Filter dependencies by name using glob syntax (e.g. "*okhttp*")
        #[arg(long, value_name = "GLOB")]
        filter: Option<String>,

        /// Only update to stable versions (no alpha, beta, RC) - enabled by default
        #[arg(short, long, default_value_t = true)]
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

    /// Add a dependency or plugin entry to the version catalog
    Add {
        /// Treat the coordinate as a plugin (plugin.id:version)
        #[arg(short = 'p', long = "plugin", conflicts_with = "library")]
        plugin: bool,

        /// Treat the coordinate explicitly as a library (group:artifact:version)
        #[arg(short = 'l', long = "library", conflicts_with = "plugin")]
        library: bool,

        /// Coordinate (library: group:artifact:version | plugin: id:version)
        #[arg(value_name = "COORDINATE")]
        coordinate: String,

        /// Override the generated alias for the catalog entry
        #[arg(long)]
        alias: Option<String>,

        /// Override the generated version alias to insert into [versions]
        #[arg(long = "version-alias")]
        version_alias: Option<String>,
    },
}
