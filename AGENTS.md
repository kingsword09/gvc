# GVC Agents Guide

This document summarizes the agent-style components that drive `gvc` and how
they cooperate with the surrounding modules. It is intended as a quick on-ramp
for contributors who want to understand or extend the existing automation.

## High-Level Workflow

- CLI entry point: `src/main.rs` wires Clap commands to `workflow::execute_*`.
- Shared flow across commands:
  1. `ProjectScannerAgent` validates the Gradle project layout and finds
     `gradle/libs.versions.toml`.
  2. `GradleConfigParser` collects repository definitions to inform network
     fetches.
  3. `DependencyUpdater` (check/update) or inline helpers (list) read the
     catalog, resolve versions, and produce an `UpdateReport`.
  4. `VersionControlAgent` optionally enforces a clean working tree and creates
     a branch plus commit when updates are applied.
- `execute_update` handles user interaction, filtering, and Git operations.
- `execute_check` shares the validation and resolution path but skips writes.
- `execute_list` presents the current catalog without touching remote services.

## Agents

### ProjectScannerAgent (`src/agents/project_scanner.rs`)

- Validates Gradle wrapper existence (`gradlew` / `gradlew.bat`) and the
  presence of `gradle/libs.versions.toml`.
- Detects whether the project is inside a Git repository so downstream steps
  can decide whether to run Git commands.
- Exposes `ProjectInfo` with resolved paths used by other components.
- Typical failure modes bubble up as `GvcError::ProjectValidation`.

### DependencyUpdater (`src/agents/dependency_updater.rs`)

- Central engine for inspecting and mutating the version catalog.
- Coordinates three main concerns:
  - Version resolution through `MavenRepository` and `PluginPortalClient`.
  - TOML parsing/editing with `toml_edit::DocumentMut`.
  - Optional interactive prompts for targeted or bulk updates.
- Produces `UpdateReport` structures summarizing changes across `[versions]`,
  `[libraries]`, and `[plugins]`.
- Supports filtering via glob patterns and stability gating (`stable_only`).
- Shares pure read path (`check_for_updates`) with update logic to avoid code
  duplication.

### CatalogEditor (`src/agents/catalog_editor.rs`)

- Lightweight helper dedicated to structural edits on `libs.versions.toml`.
- Provides `add_library` and `add_plugin` for the `gvc add` workflow, including
  alias generation and version-key insertion.
- Normalizes catalog aliases (filters common prefixes, deduplicates tokens)
  so new entries follow the same naming style as existing ones.
- Ensures both `[versions]` and the target section exist, creating them when
  needed before persisting the updated document.
- Guards against duplicate aliases or pre-existing coordinates in the catalog;
  remote resolution is performed in the workflow before these helpers persist
  any edits.

### VersionControlAgent (`src/agents/version_control.rs`)

- Wraps shell `git` commands to keep the CLI logic cohesive.
- Provides:
  - `is_working_directory_clean` guard before any writes.
  - `commit_to_new_branch`, which creates `deps/update-YYYY-MM-DD`, stages
    `gradle/libs.versions.toml`, and commits with a standard message.
- Uses the `jiff` crate to stamp branch names with the current date.
- Errors surface as `GvcError::GitOperation` so caller can print actionable
  messages.

## Supporting Modules

- `src/workflow.rs`: orchestrates agent calls, terminal output, and branching
  between `check`, `update`, `list`, and `add`; the `add` path also loads Gradle
  repository definitions, resolves `:latest` coordinates (preferring stable
  releases unless `--no-stable-only` is specified), and verifies coordinates
  against Maven repositories or the Gradle Plugin Portal before invoking
  `CatalogEditor`.
- `src/gradle/config_parser.rs`: extracts repository URLs from Gradle Kotlin or
  Groovy DSL files and falls back to sensible defaults (Maven Central, Google
  Maven, Gradle Plugin Portal).
- `src/maven/repository.rs` and `plugin_portal.rs`: perform blocking HTTP
  queries, apply repository-scoped filters, and return sorted version lists.
- `src/maven/version.rs`: normalizes semantic, numeric, snapshot, and ad-hoc
  versions to support stability checks and ordering.
- `src/cli.rs`: defines user-facing commands, global flags (e.g. `--verbose`),
  and argument parsing via Clap.

## Control Flow Notes

- `GVC_VERBOSE=1` (set via `--verbose`) enables extra logging inside network
  clients and dependency resolution routines.
- `UpdateReport::is_empty` allows `workflow::execute_update` to skip Git work
  when no upgrades are produced.
- Interaction helpers inside `DependencyUpdater` can be reused to embed custom
  UX flows (e.g. selecting from a list of candidate upgrades).

## Extending the System

- To add a new agent-like component, expose it in `src/agents/mod.rs` and wire
  it through `workflow.rs` so the CLI remains the single integration point.
- Prefer returning rich structs or enums instead of printing directly; this
  keeps workflow responsibilities focused on presentation.
- Keep network-facing code in the `maven` module to centralize HTTP settings
  and caching strategies if added later.
- When adding new interactive steps, ensure non-interactive defaults continue
  to work for batch execution (CI, scripts).

## Developer Checklist

- Run `cargo fmt` and `cargo clippy --all-targets --all-features` before
  submitting changes.
- Use `cargo test` to exercise unit tests (e.g. Maven plugin helpers, version
  parsing).
- For manual verifications, point `gvc --path <gradle-project>` to a sample
  project and compare results between `check` and `update`.
- Document new behavior in `README.md` and consider mirroring updates to
  `README_ZH.md` for parity.
