# GVC (Gradle Version Catalog Updater)

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

- Rust 1.70+ (for building)
- A Gradle project using version catalogs (`gradle/libs.versions.toml`)
- Git (optional, for branch/commit features)
- Internet connection (to query Maven repositories)

## Installation

### From source

```bash
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

Apply dependency updates:

```bash
gvc update
```

#### Options

- `--stable-only` - Only update to stable versions (default behavior)
- `--no-git` - Skip Git operations (no branch/commit)
- `--path`, `-p` - Specify project directory

**Examples:**

```bash
# Update to stable versions only (default)
gvc update --stable-only

# Update without Git integration
gvc update --no-git

# Update a specific project
gvc update --path /path/to/project
```

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

MIT

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

## Roadmap

- [ ] Async HTTP requests for concurrent version queries
- [ ] Local caching of Maven metadata
- [ ] Interactive TUI mode for selective updates
- [ ] Support for Gradle plugin updates
- [ ] Configuration file support (`.gvcrc`)
- [ ] Better error messages with suggestions
