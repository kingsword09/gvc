use crate::error::{GvcError, Result};
use colored::Colorize;
use std::fmt;
use std::io::{self, Write};

/// Categories of updates for user interaction
#[derive(Copy, Clone)]
enum UpdateCategory {
    Version,
    Library,
    Plugin,
}

impl fmt::Display for UpdateCategory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let label = match self {
            UpdateCategory::Version => "Version",
            UpdateCategory::Library => "Library",
            UpdateCategory::Plugin => "Plugin",
        };
        f.write_str(label)
    }
}

/// Manages user interaction for update operations
///
/// This component handles all user prompts and confirmations during
/// interactive update operations, separating UI concerns from business logic.
pub struct UpdateInteraction {
    enabled: bool,
    apply_all: bool,
}

impl UpdateInteraction {
    /// Create a new interaction manager
    pub fn new(enabled: bool) -> Self {
        Self {
            enabled,
            apply_all: false,
        }
    }

    /// Check if interaction is enabled
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Confirm a version update with the user
    pub fn confirm_version(&mut self, name: &str, old: &str, new: &str) -> Result<bool> {
        self.confirm(UpdateCategory::Version, name, old, new)
    }

    /// Confirm a library update with the user
    pub fn confirm_library(&mut self, name: &str, old: &str, new: &str) -> Result<bool> {
        self.confirm(UpdateCategory::Library, name, old, new)
    }

    /// Confirm a plugin update with the user
    pub fn confirm_plugin(&mut self, name: &str, old: &str, new: &str) -> Result<bool> {
        self.confirm(UpdateCategory::Plugin, name, old, new)
    }

    /// Internal confirm method
    fn confirm(
        &mut self,
        category: UpdateCategory,
        name: &str,
        old: &str,
        new: &str,
    ) -> Result<bool> {
        if !self.enabled {
            return Ok(true);
        }

        let category_label = format!("[{}]", category);
        println!(
            "\n{} {} {} {} to {}",
            category_label.cyan().bold(),
            name.white().bold(),
            "from".dimmed(),
            old.red(),
            new.green().bold()
        );

        if self.apply_all {
            println!("{}", "Auto-applying (previously selected 'all').".dimmed());
            return Ok(true);
        }

        loop {
            print!("{}", "Apply this update? [Y/n/a/q]: ".bold());
            io::stdout().flush()?;

            let mut input = String::new();
            io::stdin().read_line(&mut input)?;
            let decision = input.trim().to_lowercase();

            match decision.as_str() {
                "" | "y" | "yes" => {
                    return Ok(true);
                }
                "n" | "no" => {
                    println!("{}", "Skipping this update.".dimmed());
                    return Ok(false);
                }
                "a" | "all" => {
                    println!(
                        "{}",
                        "Applying this and all remaining updates.".green().bold()
                    );
                    self.apply_all = true;
                    return Ok(true);
                }
                "q" | "quit" => {
                    println!("{}", "Stopping update process at user request.".yellow());
                    return Err(GvcError::UserCancelled);
                }
                _ => {
                    println!(
                        "{}",
                        "Please answer with y(es), n(o), a(ll), or q(quit).".red()
                    );
                }
            }
        }
    }
}
