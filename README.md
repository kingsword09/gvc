# GVC (Gradle Version Catalog Manager)

[![Crates.io](https://img.shields.io/crates/v/gvc.svg)](https://crates.io/crates/gvc)
[![License](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-stable-orange.svg)](https://www.rust-lang.org)

A fast, standalone CLI for managing Gradle version catalogs (`libs.versions.toml`): check, list, explain, update, add, audit, and diagnose dependencies or plugins with confidence.

English | [简体中文](README_ZH.md)

## Features

- 🚀 **Direct Maven repository queries** - No Gradle runtime needed, pure Rust performance
- 📦 **Multi-repository support** - Maven Central, Google Maven, custom repositories with smart filtering
- 🎯 **Intelligent version detection** - Semantic versioning with stability filtering (alpha, beta, RC, dev)
- 📋 **Eight commands**:
  - `check` - View available updates without applying
  - `outdated` - Show outdated entries in a package-manager style table
  - `update` - Apply dependency updates
  - `list` - Display all dependencies in Maven coordinate format
  - `why` - Explain a catalog entry by alias or coordinate
  - `audit` - Find catalog quality issues such as duplicate coordinates or missing version refs
  - `add` - Insert dependencies or plugins directly into the catalog with version aliasing
  - `doctor` - Diagnose Kotlin/Android catalog consistency
- 🔒 **Version reference support** - Handles `[versions]` table with automatic resolution
- 🎨 **Beautiful CLI output** - Progress bars, colored output, clear summaries
- ⚡ **Smart request optimization** - Repository filtering based on group patterns to minimize HTTP requests

## Prerequisites

- Rust stable (for building from source)
- A Gradle project using version catalogs (`gradle/libs.versions.toml`)
- Git (optional, for branch/commit features)
- Internet connection (to query Maven repositories)

## Installation

### From crates.io (Recommended)

```bash
cargo install gvc
```

### From GitHub Releases

Download pre-built binaries from the [releases page](https://github.com/kingsword09/gvc/releases):

```bash
# Linux/macOS
curl -LO https://github.com/kingsword09/gvc/releases/download/v0.2.0/gvc-linux-x86_64
curl -LO https://github.com/kingsword09/gvc/releases/download/v0.2.0/gvc-linux-x86_64.sha256
shasum -a 256 -c gvc-linux-x86_64.sha256
chmod +x gvc-linux-x86_64
sudo mv gvc-linux-x86_64 /usr/local/bin/gvc
```

### From source

```bash
git clone https://github.com/kingsword09/gvc.git
cd gvc
cargo install --path .
```

Or build manually:

```bash
cargo build --release
# Binary will be in target/release/gvc
```

### Agent skill (optional)

This repository includes a `gvc` agent skill that can be installed with the Vercel `skills` CLI:

```bash
npx skills add kingsword09/gvc --skill gvc -g -y
```

From a local checkout:

```bash
npx skills add . --skill gvc -g -y
```

The skill teaches agents how to use `gvc`; install the CLI binary separately with `cargo install gvc` or `cargo install --path .`.

## Quick Start

```bash
gvc check              # validate project and list available upgrades
gvc outdated           # show outdated catalog entries in a table
gvc why androidx-core  # explain a catalog entry by alias or coordinate
gvc audit              # inspect catalog quality without network access
gvc update --no-git    # apply upgrades without creating a Git branch
gvc check --format json --fail-on-updates  # agent/CI-friendly update gate
gvc doctor --format json --fail-on-issues  # Kotlin/Android catalog diagnostics
```

- Use `--path /path/to/project` if the catalog lives elsewhere; global flags may be placed before or after the command.
- Use `--catalog gradle/custom.versions.toml` to target a specific catalog file inside the project.
- Pass `--verbose` (or export `GVC_VERBOSE=1`) to inspect HTTP traffic, caching, and other diagnostics.

## Usage

### Command Reference

| Command | Purpose | Key Flags |
| --- | --- | --- |
| `gvc check` | Dry-run scan that validates the project and prints available dependency/plugin upgrades. | `--include-unstable` to add alpha/beta/RC versions; `--path` to target another project. |
| `gvc outdated` | Prints outdated version aliases, libraries, and plugins in a package-manager style table. | `--include-unstable` to include pre-releases; `--fail-on-updates` exits with code 2 for automation. |
| `gvc update` | Applies or previews catalog updates, honoring stability filters and optional Git integration. | `--dry-run` to preview; `--apply` to be explicit; `--target "*glob*"` for targeted upgrades; `--no-git` to skip branch/commit; `--no-stable-only` to include pre-releases. |
| `gvc list` | Displays the resolved version catalog as Maven coordinates for quick auditing. | `--path` to point at another project. |
| `gvc why <query>` | Explains a catalog entry's coordinate, version source, duplicate aliases, and recommendations. | Query by alias, library coordinate (`group:artifact`), or plugin id; `--format json` for automation. |
| `gvc audit` | Checks catalog maintainability without network access. | `--fail-on-issues` exits with code 2 when warnings/errors are found; `--format json` for automation. |
| `gvc doctor` | Checks Kotlin, KSP, Android Gradle Plugin, and Compose catalog consistency without network access. | `--fail-on-issues` exits with code 2 when warnings/errors are found; `--format json` for automation. |
| `gvc add` | Inserts a new entry into `[libraries]` (default) or `[plugins]`. | `-P/--plugin` targets plugins; `--no-stable-only` allows pre-releases when resolving `:latest`; `--alias` / `--version-alias` override generated keys. |

Global automation flags:

- `--format text|json` - Emits either human-readable output or a stable JSON object.
- `--quiet` - Suppresses progress output in text mode.
- `--no-color` - Disables ANSI color.
- `--catalog <file>` - Uses an explicit version catalog file under `--path`.

Exit codes:

- `0` - Command completed successfully.
- `1` - Validation, parsing, network, Git, or write error.
- `2` - `gvc check/outdated --fail-on-updates` found updates, or `gvc audit/doctor --fail-on-issues` found diagnostics.

### Check for Updates

View available dependency updates without modifying any files:

```bash
gvc check
# or
gvc --path /path/to/project check
```

By default, only stable versions are shown. To include pre-release versions:

```bash
gvc check --include-unstable
```

For CI or agents that should fail when upgrades exist:

```bash
gvc check --format json --fail-on-updates
```

### Show Outdated Entries

Use `outdated` when you want a compact package-manager style view of what can move:

```bash
gvc outdated
gvc outdated --include-unstable
gvc outdated --format json --fail-on-updates
```

`outdated` uses the same resolver as `check`, so it respects the Gradle repositories configured for the project and the same stable-version filtering rules.

### Audit Catalog Quality

Run an offline maintainability audit for the version catalog:

```bash
gvc audit
gvc audit --format json
gvc audit --fail-on-issues
```

The audit currently checks:

- `version.ref` values that point to missing `[versions]` aliases.
- Multiple aliases pointing to the same library or plugin coordinate.
- Inline versions that could be moved into `[versions]`.
- `[versions]` aliases not referenced by catalog libraries or plugins.
- Multiple `[versions]` aliases with the same value.

### List Dependencies

Display all dependencies in Maven coordinate format (useful for verification):

```bash
gvc list
gvc list --format json
```

Output example:
```
📦 Dependencies:

Libraries:
  androidx.core:core-ktx:1.12.0
  com.squareup.okhttp3:okhttp:4.12.0
  org.jetbrains.compose.runtime:runtime:1.9.0

Plugins:
  org.jetbrains.kotlin.jvm:1.9.0
  com.android.application:8.1.0

Summary:
  4 libraries
  2 plugins
```

### Explain a Catalog Entry

Use `why` to inspect how an alias or coordinate is declared and resolved:

```bash
gvc why androidx-core
gvc why androidx.core:core-ktx
gvc why com.android.application --format json
```

The report shows the matched entry, coordinate, inline version or `version.ref`, resolved version, duplicate aliases for the same coordinate, and any low-risk recommendations.

### Diagnose Kotlin/Android Catalogs

Run catalog-only diagnostics for Kotlin-heavy Gradle projects:

```bash
gvc doctor
gvc doctor --format json
gvc doctor --fail-on-issues
```

The doctor currently checks:

- Kotlin Gradle plugin entries use one aligned version.
- KSP versions use the expected `<kotlin-version>-<ksp-version>` prefix.
- Kotlin 2.x Compose projects declare `org.jetbrains.kotlin.plugin.compose`.
- The Kotlin Compose compiler plugin matches the Kotlin Gradle plugin version.
- `com.android.*` plugins share one Android Gradle Plugin version.

`doctor` is intentionally offline and only inspects the version catalog, so it is safe for CI and agent workflows.

### Update Dependencies

Apply dependency updates (stable versions only by default):

```bash
gvc update
```

#### Options

- `--stable-only` - Only update to stable versions (enabled by default)
- `--no-stable-only` - Allow updates to unstable versions (alpha, beta, RC)
- `--dry-run` - Preview updates without writing to the catalog
- `--apply` - Explicitly apply updates (default behavior)
- `--target <glob>` - Limit updates to dependencies whose alias matches the glob (e.g. `*okhttp*`)
- `-i`, `--interactive` - Review each proposed change before applying it
- `--filter <glob>` - Backward-compatible alias for `--target`
- `--no-git` - Skip Git operations (no branch/commit)
- `--path`, `-p` - Specify project directory
- `--format json` - Emit the applied update report as JSON

Interactive mode will pause on each candidate upgrade, showing the old/new version and letting you accept, skip, apply all remaining changes, or cancel the run.

Dry-run mode is read-only and does not require a clean Git working tree:

```bash
gvc update --dry-run
gvc update --dry-run --target kotlin --format json
```

#### Targeted Updates

When `--target` is provided in apply mode, GVC lists every matching library/version alias/plugin so you can pick a single target. Combine it with `-i/--interactive` to choose the exact version (stable or pre-release) you want to install.

```bash
# Review and pick a version for dependencies with "okhttp" in their alias
gvc update --target "*okhttp*" --interactive
```

- Skip the version prompt by omitting `--interactive` when the target pattern matches exactly one entry; GVC selects the newest version that satisfies the stability rules. If multiple entries match, refine the target or add `--interactive`.
- Include pre-releases with `--no-stable-only` when you want to evaluate beta/RC builds.

**Examples:**

```bash
# Update to stable versions only (default behavior)
gvc update

# Include unstable versions (alpha, beta, RC)
gvc update --no-stable-only

# Review each update before writing changes
gvc update --interactive

# Target a single dependency by alias pattern
gvc update --target "*okhttp*"

# Update without Git integration
gvc update --no-git

# Update a specific project
gvc update --path /path/to/project
```

#### Selective Updates

When you pass `--target`, GVC narrows the scope to aliases that match your glob expression (case-insensitive). The CLI will:

1. List every matching version alias, library, or plugin.
2. Prompt you to pick the exact entry to change.
3. Fetch available versions from the configured repositories.
4. In interactive mode (`-i`), let you choose from recent stable and pre-release versions (use `m` to show more, `s` to skip, `q` to cancel).
5. Without interactive mode, automatically pick the first newer version when exactly one entry matches. If multiple entries match, GVC exits with a message asking for a narrower target or `--interactive`.

This makes it easy to bump a single dependency—even to a specific pre-release—without touching the rest of the catalog.

### Add Dependencies or Plugins

Create new catalog entries directly from Maven or plugin coordinates:

```bash
# Libraries: group:artifact:version (default target)
gvc add androidx.lifecycle:lifecycle-runtime-ktx:2.6.2

# Plugins: plugin.id:version
gvc add -P org.jetbrains.kotlin.jvm:1.9.24

# Resolve the newest available version automatically
gvc add com.squareup.okhttp3:okhttp:latest
gvc add -P org.jetbrains.kotlin.android:latest --no-stable-only  # allow pre-releases when needed
```

- GVC auto-generates catalog aliases and version keys (use `--alias` / `--version-alias` to override).
- `-P` is the plugin short flag; global `-p/--path` is reserved for the project path.
- Library entries are written as `{ module = "group:artifact", version = { ref = "<alias>" } }`.
- Plugin entries use `{ id = "plugin.id", version = { ref = "<alias>" } }`.
- Existing version aliases are not updated implicitly. Pass `--update-version-alias` only when you intentionally want the new entry to move an existing version key.
- Coordinates are verified upstream before writing; libraries query your configured repositories, plugins query the Gradle Plugin Portal. Use `--no-stable-only` to include pre-release versions when resolving `:latest`.
- The `--path` flag works exactly as with other commands.

## How It Works

GVC directly queries Maven repositories without requiring Gradle:

1. **Project Validation** - Checks for `gradle/libs.versions.toml` and `gradlew`
2. **Repository Configuration** - Reads Gradle build files to detect configured Maven repositories
3. **TOML Parsing** - Uses `toml_edit` to parse version catalog while preserving formatting
4. **Version Resolution**:
   - Parses dependencies in all supported TOML formats
   - Resolves version references from `[versions]` table
   - Queries Maven repositories for latest versions via HTTP
   - Applies smart filtering based on repository group patterns
5. **Version Comparison**:
   - Semantic versioning support (1.0.0, 2.1.3)
   - Filters unstable versions (alpha, beta, RC, dev, snapshot, preview, etc.)
   - Prevents version downgrades
6. **Update Application** - Updates TOML file while maintaining original formatting

### Supported TOML Formats

GVC supports all Gradle version catalog formats:

```toml
# Simple string format
[libraries]
okhttp = "com.squareup.okhttp3:okhttp:4.11.0"

# Table format with module
okhttp = { module = "com.squareup.okhttp3:okhttp", version = "4.11.0" }

# Table format with group and name
okhttp = { group = "com.squareup.okhttp3", name = "okhttp", version = "4.11.0" }

# Version references (automatically resolved)
[versions]
okhttp = "4.11.0"

[libraries]
okhttp = { group = "com.squareup.okhttp3", name = "okhttp", version.ref = "okhttp" }
```

### Smart Repository Filtering

GVC automatically filters repository requests based on dependency group:

- **Google Maven** - Only queries for `google.*`, `android.*`, `androidx.*` packages
- **Maven Central** - Queries for all other packages
- **Custom Repositories** - Respects `mavenContent.includeGroupByRegex` patterns

This significantly reduces unnecessary HTTP requests and speeds up checks.

## Architecture Overview

- Workflows in `src/workflow.rs` orchestrate CLI commands, progress output, and Git handoff.
- Agents encapsulate core responsibilities:
  - `ProjectScannerAgent` validates Gradle structure and locates `libs.versions.toml`.
  - `DependencyUpdater` reads, evaluates, and mutates the catalog with repository-aware version lookups.
  - `VersionControlAgent` guards Git cleanliness and creates update branches plus commits when enabled.
- See [AGENTS.md](AGENTS.md) for a deeper dive into responsibilities, extension tips, and developer checklists.

## Project Requirements

Your Gradle project must have:

1. **Version catalog file**: `gradle/libs.versions.toml`
2. **Gradle wrapper**: `gradlew` or `gradlew.bat` (for repository detection)

**No Gradle plugins required!** GVC directly queries Maven repositories and updates your TOML file.

## Repository Detection

GVC automatically reads repository configuration from your Gradle build files:

- `settings.gradle.kts` / `settings.gradle`
- `build.gradle.kts` / `build.gradle`

Detected repositories:
- `mavenCentral()`
- `google()`
- `gradlePluginPortal()`
- Custom `maven { url = "..." }` declarations
- Repository content filters (`mavenContent.includeGroupByRegex`)

## Examples

### Check for Updates

```bash
$ gvc check

Checking for available updates (stable versions)...

1. Validating project structure...
✓ Project structure is valid

2. Reading Gradle repository configuration...
   Found 3 repositories:
   • Maven Central (https://repo1.maven.org/maven2)
   • Google Maven (https://dl.google.com/dl/android/maven2)
   • Gradle Plugin Portal (https://plugins.gradle.org/m2)

3. Checking for available updates...

Checking version variables...
[========================================] 10/10

Checking library updates...
[========================================] 25/25

✓ Check completed

📦 Available Updates:
Found 5 update(s)
   (showing stable versions only)

Version updates:
  • okio-version 3.16.0 → 3.16.2
  • kotlin-version 2.2.20 → 2.2.21
  • ktor-version 3.3.0 → 3.3.1

Library updates:
  • some-direct-lib 0.9.0 → 0.10.0 (stable)

To apply these updates, run:
  gvc update --stable-only
```

### List All Dependencies

```bash
$ gvc list

Listing dependencies in version catalog...

1. Validating project structure...
✓ Project structure is valid

2. Reading version catalog...
✓ Catalog loaded

📦 Dependencies:

Libraries:
  androidx.core:core-ktx:1.17.0
  com.squareup.okhttp3:okhttp:4.12.0
  io.ktor:ktor-server-core:3.3.0
  org.jetbrains.compose.runtime:runtime:1.9.0

Plugins:
  org.jetbrains.kotlin.jvm:2.2.20
  com.android.application:8.13.0

Summary:
  4 libraries
  2 plugins
```

## Troubleshooting

### "Gradle wrapper not found"

Ensure your project has `gradlew` (Linux/Mac) or `gradlew.bat` (Windows) in the root directory.

### "gradle/libs.versions.toml not found"

Make sure your project uses Gradle version catalogs and the file exists at `gradle/libs.versions.toml`.

### "Working directory has uncommitted changes"

Commit or stash your changes before running the update command, or use `--no-git` to skip Git operations.

## Development

### Project Structure

```
gvc/
├── src/
│   ├── main.rs              # Entry point
│   ├── cli.rs               # CLI argument parsing
│   ├── workflow.rs          # Command orchestration
│   ├── error.rs             # Error types
│   ├── agents/
│   │   ├── dependency_updater.rs  # Core update logic
│   │   ├── project_scanner.rs     # Project validation
│   │   └── version_control.rs     # Git operations
│   ├── gradle/
│   │   └── config_parser.rs       # Gradle configuration parsing
│   └── maven/
│       ├── repository.rs          # Maven HTTP client
│       ├── version.rs             # Version comparison
│       └── mod.rs                 # Maven coordinate parsing
├── Cargo.toml
└── README.md
```

### Building

```bash
# Development
cargo build

# Release (optimized)
cargo build --release
```

### Testing

```bash
cargo test
```

### Running in development

```bash
# Check updates
cargo run -- check

# List dependencies
cargo run -- list

# Update dependencies
cargo run -- update --no-git
```

See [AGENTS.md](AGENTS.md) for an in-depth guide to the agent modules that power these workflows.

## License

Apache-2.0

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

### Development Setup

1. Clone the repository:
   ```bash
   git clone https://github.com/kingsword09/gvc.git
   cd gvc
   ```

2. Build and test:
   ```bash
   cargo build
   cargo test
   cargo fmt
   cargo clippy --all-targets --all-features
   ```

3. Run locally:
   ```bash
   cargo run -- check
   cargo run -- update --no-git
   ```

See [CONTRIBUTING.md](CONTRIBUTING.md) for more details.

## Changelog

See [CHANGELOG.md](CHANGELOG.md) for release history.

## Roadmap

- [ ] Async HTTP requests for concurrent version queries
- [ ] Local caching of Maven metadata
- [ ] Interactive TUI mode for selective updates
- [x] Support for Gradle plugin updates (Gradle Plugin Portal integration) ✅
- [ ] Configuration file support (`.gvcrc`)
- [ ] Better error messages with suggestions
