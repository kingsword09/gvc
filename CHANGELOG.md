# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0] - 2025-10-25

### Added
- Initial release of GVC (Gradle Version Catalog Updater)
- Three main commands: `check`, `list`, and `update`
- Direct Maven repository queries (Maven Central, Google Maven)
- Smart repository filtering based on group patterns
- Version reference resolution from `[versions]` table
- Support for all Gradle version catalog TOML formats
- Semantic versioning with stability detection
- Gradle configuration auto-detection
- Repository content filter support (`includeGroupByRegex`)
- Prevention of version downgrades
- Comprehensive documentation (English and Chinese)
- GitHub Actions CI/CD workflows

### Technical Details
- Built with Rust 2024 edition
- Uses `toml_edit` for TOML parsing with format preservation
- HTTP-based Maven metadata fetching and XML parsing
- Intelligent version comparison and filtering
- Progress bars and colored CLI output

[Unreleased]: https://github.com/kingsword09/gvc/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/kingsword09/gvc/releases/tag/v0.1.0
