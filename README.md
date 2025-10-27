# GVC (Gradle Version Catalog Updater)

[![Crates.io](https://img.shields.io/crates/v/gvc.svg)](https://crates.io/crates/gvc)
[![License](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-stable-orange.svg)](https://www.rust-lang.org)

A fast, standalone CLI tool for checking and updating Gradle dependencies in version catalogs (`libs.versions.toml`).

English | [简体中文](README_ZH.md)

## Features

- 🚀 **Direct Maven repository queries** - No Gradle runtime needed, pure Rust performance
- 📦 **Multi-repository support** - Maven Central, Google Maven, custom repositories with smart filtering
- 🎯 **Intelligent version detection** - Semantic versioning with stability filtering (alpha, beta, RC, dev)
- 📋 **Three commands**:
  - `check` - View available updates without applying
  - `update` - Apply dependency updates
  - `list` - Display all dependencies in Maven coordinate format
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
curl -L https://github.com/kingsword09/gvc/releases/download/v0.1.0/gvc-x86_64-unknown-linux-gnu -o gvc
chmod +x gvc
sudo mv gvc /usr/local/bin/

# Or use the install script
curl -sSL https://raw.githubusercontent.com/kingsword09/gvc/main/install.sh | bash
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

## Usage

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

### List Dependencies

Display all dependencies in Maven coordinate format (useful for verification):

```bash
gvc list
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

### Update Dependencies

Apply dependency updates (stable versions only by default):

```bash
gvc update
```

#### Options

- `--stable-only` - Only update to stable versions (enabled by default)
- `--no-stable-only` - Allow updates to unstable versions (alpha, beta, RC)
- `-i`, `--interactive` - Review each proposed change before applying it
- `--filter <glob>` - Limit updates to dependencies whose alias matches the glob (e.g. `*okhttp*`)
- `--no-git` - Skip Git operations (no branch/commit)
- `--path`, `-p` - Specify project directory

Interactive mode will pause on each candidate upgrade, showing the old/new version and letting you accept, skip, apply all remaining changes, or cancel the run.

When `--filter` is provided, GVC lists every matching library/version alias/plugin so you can pick a single target. Combine it with `-i/--interactive` to choose the exact version (stable or pre-release) you want to install.

**Examples:**

```bash
# Update to stable versions only (default behavior)
gvc update

# Include unstable versions (alpha, beta, RC)
gvc update --no-stable-only

# Review each update before writing changes
gvc update --interactive

# Target a single dependency by alias pattern
gvc update --filter "*okhttp*"

# Update without Git integration
gvc update --no-git

# Update a specific project
gvc update --path /path/to/project
```

#### Selective Updates

When you pass `--filter`, GVC narrows the scope to aliases that match your glob expression (case-insensitive). The CLI will:

1. List every matching version alias, library, or plugin.
2. Prompt you to pick the exact entry to change.
3. Fetch available versions from the configured repositories.
4. In interactive mode (`-i`), let you choose from recent stable and pre-release versions (use `m` to show more, `s` to skip, `q` to cancel).
5. Without interactive mode, automatically pick the first newer version that respects the `--stable-only` flag, so you can script targeted upgrades.

This makes it easy to bump a single dependency—even to a specific pre-release—without touching the rest of the catalog.

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
   cargo clippy
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
