use clap::{Parser, Subcommand, ValueEnum};

#[derive(Parser, Debug)]
#[command(
    name = "gvc",
    about = "Gradle Version Catalog - A tool to manage Gradle dependency updates",
    version,
    author
)]
pub struct Cli {
    /// Path to the project directory (defaults to current directory)
    #[arg(short, long, global = true, default_value = ".")]
    pub path: String,

    /// Path to the version catalog file (defaults to gradle/libs.versions.toml)
    #[arg(long, global = true, value_name = "FILE")]
    pub catalog: Option<String>,

    /// Output format
    #[arg(long, global = true, value_enum, default_value_t = OutputFormat::Text)]
    pub format: OutputFormat,

    /// Disable colored output
    #[arg(long, global = true)]
    pub no_color: bool,

    /// Suppress human-readable progress output
    #[arg(short = 'q', long, global = true)]
    pub quiet: bool,

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
        #[arg(long, value_name = "GLOB", conflicts_with = "target")]
        filter: Option<String>,

        /// Target dependencies by alias using glob syntax (agent-friendly alias for --filter)
        #[arg(long, value_name = "GLOB")]
        target: Option<String>,

        /// Only update to stable versions (no alpha, beta, RC) - enabled by default
        #[arg(short, long, action = clap::ArgAction::SetTrue, conflicts_with = "no_stable_only")]
        stable_only: bool,

        /// Include unstable versions (alpha, beta, RC)
        #[arg(long = "no-stable-only", action = clap::ArgAction::SetTrue)]
        no_stable_only: bool,

        /// Skip Git operations (don't create branch or commit)
        #[arg(long)]
        no_git: bool,

        /// Preview updates without writing to the catalog
        #[arg(long, conflicts_with = "apply")]
        dry_run: bool,

        /// Explicitly apply updates (default behavior)
        #[arg(long)]
        apply: bool,
    },

    /// Check for available dependency updates without applying them
    Check {
        /// Include unstable versions (alpha, beta, RC)
        #[arg(long)]
        include_unstable: bool,

        /// Exit with code 2 when updates are available
        #[arg(long)]
        fail_on_updates: bool,
    },

    /// Show outdated catalog entries in a package-manager style table
    Outdated {
        /// Include unstable versions (alpha, beta, RC)
        #[arg(long)]
        include_unstable: bool,

        /// Exit with code 2 when updates are available
        #[arg(long)]
        fail_on_updates: bool,
    },

    /// List all dependencies in the version catalog
    List,

    /// Explain a catalog entry by alias or coordinate
    Why {
        /// Alias, library coordinate (group:artifact), or plugin id to explain
        #[arg(value_name = "QUERY")]
        query: String,
    },

    /// Audit version catalog quality and maintainability
    Audit {
        /// Exit with code 2 when warnings or errors are found
        #[arg(long)]
        fail_on_issues: bool,
    },

    /// Diagnose Kotlin/Android version catalog consistency
    Doctor {
        /// Exit with code 2 when warnings or errors are found
        #[arg(long)]
        fail_on_issues: bool,
    },

    /// Add a dependency or plugin entry to the version catalog
    Add {
        /// Treat the coordinate as a plugin (plugin.id:version)
        #[arg(short = 'P', long = "plugin", conflicts_with = "library")]
        plugin: bool,

        /// Treat the coordinate explicitly as a library (group:artifact:version)
        #[arg(short = 'l', long = "library", conflicts_with = "plugin")]
        library: bool,

        /// Coordinate (library: group:artifact:version|latest, plugin: id:version|latest)
        #[arg(value_name = "COORDINATE")]
        coordinate: String,

        /// Override the generated alias for the catalog entry
        #[arg(long)]
        alias: Option<String>,

        /// Override the generated version alias to insert into [versions]
        #[arg(long = "version-alias")]
        version_alias: Option<String>,

        /// Include unstable versions when resolving `:latest` coordinates
        #[arg(long = "no-stable-only", action = clap::ArgAction::SetFalse, default_value_t = true)]
        stable_only: bool,

        /// Allow an existing version alias to be updated when it has a different version
        #[arg(long)]
        update_version_alias: bool,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum OutputFormat {
    Text,
    Json,
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn update_accepts_no_stable_only_flag() {
        let cli = Cli::parse_from(["gvc", "update", "--no-stable-only"]);
        let Commands::Update { no_stable_only, .. } = cli.command else {
            panic!("expected update command");
        };

        assert!(no_stable_only);
    }

    #[test]
    fn update_accepts_target_and_dry_run_flags() {
        let cli = Cli::parse_from(["gvc", "update", "--target", "kotlin", "--dry-run"]);
        let Commands::Update {
            target, dry_run, ..
        } = cli.command
        else {
            panic!("expected update command");
        };

        assert_eq!(target.as_deref(), Some("kotlin"));
        assert!(dry_run);
    }

    #[test]
    fn update_rejects_filter_with_target() {
        let result =
            Cli::try_parse_from(["gvc", "update", "--filter", "kotlin", "--target", "ksp"]);
        assert!(result.is_err());
    }

    #[test]
    fn update_rejects_apply_with_dry_run() {
        let result = Cli::try_parse_from(["gvc", "update", "--dry-run", "--apply"]);
        assert!(result.is_err());
    }

    #[test]
    fn accepts_global_agent_output_flags() {
        let cli = Cli::parse_from([
            "gvc",
            "--path",
            "/tmp/project",
            "--format",
            "json",
            "--no-color",
            "--quiet",
            "--catalog",
            "gradle/custom.versions.toml",
            "list",
        ]);

        assert_eq!(cli.format, OutputFormat::Json);
        assert_eq!(cli.path, "/tmp/project");
        assert!(cli.no_color);
        assert!(cli.quiet);
        assert_eq!(cli.catalog.as_deref(), Some("gradle/custom.versions.toml"));
    }

    #[test]
    fn accepts_path_after_subcommand() {
        let cli = Cli::parse_from(["gvc", "check", "--path", "/tmp/project"]);
        let Commands::Check { .. } = cli.command else {
            panic!("expected check command");
        };

        assert_eq!(cli.path, "/tmp/project");
    }

    #[test]
    fn check_accepts_fail_on_updates_flag() {
        let cli = Cli::parse_from(["gvc", "check", "--fail-on-updates"]);
        let Commands::Check {
            fail_on_updates, ..
        } = cli.command
        else {
            panic!("expected check command");
        };

        assert!(fail_on_updates);
    }

    #[test]
    fn outdated_accepts_agent_flags() {
        let cli = Cli::parse_from(["gvc", "outdated", "--include-unstable", "--fail-on-updates"]);
        let Commands::Outdated {
            include_unstable,
            fail_on_updates,
        } = cli.command
        else {
            panic!("expected outdated command");
        };

        assert!(include_unstable);
        assert!(fail_on_updates);
    }

    #[test]
    fn audit_accepts_fail_on_issues_flag() {
        let cli = Cli::parse_from(["gvc", "audit", "--fail-on-issues"]);
        let Commands::Audit { fail_on_issues } = cli.command else {
            panic!("expected audit command");
        };

        assert!(fail_on_issues);
    }

    #[test]
    fn why_accepts_query() {
        let cli = Cli::parse_from(["gvc", "why", "androidxCore"]);
        let Commands::Why { query } = cli.command else {
            panic!("expected why command");
        };

        assert_eq!(query, "androidxCore");
    }

    #[test]
    fn add_accepts_update_version_alias_flag() {
        let cli = Cli::parse_from([
            "gvc",
            "add",
            "--update-version-alias",
            "com.example:lib:1.0.0",
        ]);
        let Commands::Add {
            update_version_alias,
            ..
        } = cli.command
        else {
            panic!("expected add command");
        };

        assert!(update_version_alias);
    }

    #[test]
    fn add_accepts_visible_plugin_short_flag() {
        let cli = Cli::parse_from(["gvc", "add", "-P", "org.jetbrains.kotlin.jvm:2.0.21"]);
        let Commands::Add { plugin, .. } = cli.command else {
            panic!("expected add command");
        };

        assert!(plugin);
    }

    #[test]
    fn add_rejects_legacy_plugin_short_flag() {
        let result = Cli::try_parse_from(["gvc", "add", "-p", "org.jetbrains.kotlin.jvm:2.0.21"]);

        assert!(result.is_err());
    }

    #[test]
    fn doctor_accepts_fail_on_issues_flag() {
        let cli = Cli::parse_from(["gvc", "doctor", "--fail-on-issues"]);
        let Commands::Doctor { fail_on_issues } = cli.command else {
            panic!("expected doctor command");
        };

        assert!(fail_on_issues);
    }
}
