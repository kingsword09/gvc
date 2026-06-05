---
name: gvc
description: Gradle version catalog management for AI agents. Use when working with gradle/libs.versions.toml, checking or updating Gradle dependency and plugin versions, adding catalog entries, explaining aliases, auditing catalog quality, or diagnosing Kotlin and Android version consistency with the gvc CLI.
metadata:
  priority: 3
  pathPatterns:
    - 'gradle/libs.versions.toml'
    - '**/gradle/libs.versions.toml'
    - 'gradle/*.versions.toml'
    - 'build.gradle'
    - 'build.gradle.kts'
    - 'settings.gradle'
    - 'settings.gradle.kts'
  bashPatterns:
    - '\bgvc\b'
    - '\blibs\.versions\.toml\b'
    - '\bgradle version catalog\b'
    - '\bversion catalog\b'
---

# GVC

Use `gvc` to inspect and maintain Gradle version catalogs without hand-editing `libs.versions.toml` unless the user explicitly asks for manual TOML changes.

## Core Workflow

1. Find the Gradle project root and catalog.
   - Default catalog: `gradle/libs.versions.toml`
   - For nonstandard files, pass `--catalog <path-inside-project>`
   - For another project, pass `--path <project-root>`

2. Inspect before changing.

```bash
gvc list
gvc why <alias-or-coordinate>
gvc audit
gvc doctor
```

3. Preview remote version changes before writing.

```bash
gvc check
gvc outdated
gvc update --dry-run
gvc update --dry-run --target "*kotlin*"
```

4. Apply the smallest useful change.

```bash
gvc update --target "*kotlin*" --no-git
gvc update --no-git
```

Use `--no-git` when the caller or surrounding agent workflow owns commits. Omit it only when the user wants `gvc` to create its dependency-update branch and commit.

## Adding Catalog Entries

Libraries use `group:artifact:version`:

```bash
gvc add com.squareup.okhttp3:okhttp:4.12.0
gvc add com.squareup.okhttp3:okhttp:latest
```

Plugins use `-P` and `plugin.id:version`:

```bash
gvc add -P org.jetbrains.kotlin.jvm:2.0.21
gvc add -P com.android.application:latest
```

When resolving `:latest`, `gvc` prefers stable releases. Use `--no-stable-only` only when the user asks for alpha, beta, RC, or other pre-release versions.

If generated aliases do not match the project's naming style, provide them explicitly:

```bash
gvc add com.squareup.okhttp3:okhttp:4.12.0 --alias okhttp --version-alias okhttp
```

## Agent-Friendly Output

Use JSON for automation or CI gates:

```bash
gvc check --format json --fail-on-updates
gvc outdated --format json --fail-on-updates
gvc audit --format json --fail-on-issues
gvc doctor --format json --fail-on-issues
gvc why kotlin --format json
```

Exit code `2` means updates or issues were found when a `--fail-on-*` gate is enabled. Exit code `1` means validation, parsing, network, Git, or write failure.

## Safety Rules

- Prefer `check`, `outdated`, or `update --dry-run` before applying updates.
- Prefer targeted updates with `--target` when the user names a dependency family.
- Keep stable-only behavior unless the user requests unstable versions.
- Use `why`, `audit`, and `doctor` to understand catalog structure before changing shared aliases.
- Do not rewrite unrelated catalog formatting or reorder entries by hand.

## Working On GVC Itself

Inside this repository, run the CLI through Cargo when testing local changes:

```bash
cargo run -- check --path <gradle-project>
cargo run -- update --dry-run --path <gradle-project>
```

Before finishing changes to this repo, run:

```bash
cargo fmt
cargo test
cargo clippy --all-targets --all-features
```
