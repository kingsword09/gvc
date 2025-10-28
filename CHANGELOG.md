# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.1] - 2025-10-28

### Added
- `gvc add` supports coordinates ending in `:latest`, resolving to the newest
  stable version (with `--no-stable-only` to allow pre-releases).
- Remote validation of new dependencies/plugins against configured Maven
  repositories and the Gradle Plugin Portal before writing to the catalog.

### Changed
- Updating `:latest` entries now refreshes existing version aliases instead of
  erroring when a previous alias already exists.
- Rebranded CLI messaging and metadata to “Gradle Version Catalog Manager.”

### Documentation
- Expanded README/README_ZH and AGENTS.md to cover the `add` workflow, `:latest`
  resolution, and the broader manager positioning.

## [0.1.0] - 2025-10-26

### Added
- Initial release of GVC (Gradle Version Catalog Updater)
- Three main commands: `check`, `list`, and `update`
- Direct Maven repository queries (Maven Central, Google Maven, custom repositories)
- **Gradle Plugin Portal support** - Update Gradle plugins automatically
- Smart repository filtering based on group patterns
- Version reference resolution from `[versions]` table
- Support for all Gradle version catalog TOML formats
- Semantic versioning with stability detection and filtering
- **Verbose logging mode** (`--verbose` / `-v`) for debugging
- Gradle configuration auto-detection from build files
- Repository content filter support (`includeGroupByRegex`)
- Prevention of version downgrades
- Git integration (automatic branch creation and commits)
- **Default stable-only behavior** for safe updates
- Comprehensive test coverage (17 tests)
- Comprehensive documentation (English and Chinese)
- GitHub Actions CI/CD workflows

### Features
- ✅ Library dependency updates (Maven Central, Google Maven)
- ✅ Plugin updates (Gradle Plugin Portal)
- ✅ Version variable updates (`[versions]` table)
- ✅ Multiple TOML format support (string, table, inline table)
- ✅ Version reference resolution (`version.ref`)
- ✅ Stability filtering (alpha, beta, RC, dev, snapshot, etc.)
- ✅ HTTP timeout unification (30 seconds)
- ✅ Detailed error logging with verbose mode

### Technical Details
- Built with Rust stable (2024 edition, MSRV: 1.85)
- Uses `toml_edit` for TOML parsing with format preservation
- HTTP-based Maven metadata fetching and XML parsing
- Intelligent version comparison (semantic + numeric)
- Progress bars and colored CLI output
- Comprehensive version comparison tests (edge cases covered)

### Documentation
- English README with installation instructions
- Chinese README (简体中文)
- Contributing guidelines
- Apache-2.0 license

[0.1.1]: https://github.com/kingsword09/gvc/releases/tag/v0.1.1
[0.1.0]: https://github.com/kingsword09/gvc/releases/tag/v0.1.0
