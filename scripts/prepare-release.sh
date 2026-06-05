#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
Usage: scripts/prepare-release.sh <version> [options]

Prepare a GVC release by updating the mechanical version references, refreshing
Cargo.lock, and running release checks.

Arguments:
  <version>             Release version, with or without a leading "v".

Options:
  --date YYYY-MM-DD     Release date for CHANGELOG.md. Defaults to today.
  --allow-dirty         Allow existing edits in release metadata files.
  --skip-checks         Skip cargo fmt, clippy, and tests.
  --skip-package        Skip cargo package verification.
  -h, --help            Show this help.

Examples:
  scripts/prepare-release.sh 0.3.0
  scripts/prepare-release.sh v0.3.0 --date 2026-06-05
USAGE
}

die() {
  printf 'error: %s\n' "$*" >&2
  exit 1
}

info() {
  printf '==> %s\n' "$*"
}

version=""
release_date="${RELEASE_DATE:-$(date +%F)}"
allow_dirty=0
run_checks=1
run_package=1

while (($#)); do
  case "$1" in
    -h | --help)
      usage
      exit 0
      ;;
    --date)
      [[ $# -ge 2 ]] || die "--date requires YYYY-MM-DD"
      release_date="$2"
      shift 2
      ;;
    --allow-dirty)
      allow_dirty=1
      shift
      ;;
    --skip-checks)
      run_checks=0
      shift
      ;;
    --skip-package)
      run_package=0
      shift
      ;;
    --*)
      die "unknown option: $1"
      ;;
    *)
      [[ -z "$version" ]] || die "only one version argument is allowed"
      version="${1#v}"
      shift
      ;;
  esac
done

[[ -n "$version" ]] || die "missing release version"
[[ "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z.-]+)?(\+[0-9A-Za-z.-]+)?$ ]] || \
  die "version must look like semantic versioning, for example 0.3.0"
[[ "$release_date" =~ ^[0-9]{4}-[0-9]{2}-[0-9]{2}$ ]] || \
  die "release date must use YYYY-MM-DD"

if git_root=$(git rev-parse --show-toplevel 2>/dev/null); then
  cd "$git_root"
fi

for required in Cargo.toml Cargo.lock README.md README_ZH.md CHANGELOG.md; do
  [[ -f "$required" ]] || die "required file not found: $required"
done

if [[ $allow_dirty -eq 0 ]] && git rev-parse --is-inside-work-tree >/dev/null 2>&1; then
  dirty_metadata=$(git status --short -- Cargo.toml Cargo.lock README.md README_ZH.md CHANGELOG.md || true)
  if [[ -n "$dirty_metadata" ]]; then
    printf '%s\n' "$dirty_metadata" >&2
    die "release metadata files already have edits; commit/stash them or pass --allow-dirty"
  fi
fi

current_version=$(awk -F'"' '
  /^\[package\]$/ { in_package = 1; next }
  /^\[/ && in_package { exit }
  in_package && /^version = "/ { print $2; exit }
' Cargo.toml)

[[ -n "$current_version" ]] || die "could not read package version from Cargo.toml"

if [[ "$current_version" == "$version" ]]; then
  info "Preparing existing version $version"
else
  info "Updating package version: $current_version -> $version"
fi

CURRENT_VERSION="$current_version" RELEASE_VERSION="$version" perl -0pi -e '
  s/^version = "\Q$ENV{CURRENT_VERSION}\E"/version = "$ENV{RELEASE_VERSION}"/m
' Cargo.toml

info "Updating release download URLs in README files"
CURRENT_VERSION="$current_version" RELEASE_VERSION="$version" perl -0pi -e '
  s/releases\/download\/v\Q$ENV{CURRENT_VERSION}\E/releases\/download\/v$ENV{RELEASE_VERSION}/g
' README.md README_ZH.md

info "Updating CHANGELOG.md"
if grep -Fq "## [$version]" CHANGELOG.md; then
  RELEASE_VERSION="$version" RELEASE_DATE="$release_date" perl -0pi -e '
    s/^## \[\Q$ENV{RELEASE_VERSION}\E\] - [0-9]{4}-[0-9]{2}-[0-9]{2}$/## [$ENV{RELEASE_VERSION}] - $ENV{RELEASE_DATE}/m
  ' CHANGELOG.md
else
  tmp=$(mktemp)
  awk -v version="$version" -v release_date="$release_date" '
    !inserted && /^## \[/ {
      print "## [" version "] - " release_date
      print ""
      print "### Changed"
      print "- TODO: Summarize release changes."
      print ""
      inserted = 1
    }
    { print }
    END {
      if (!inserted) {
        print ""
        print "## [" version "] - " release_date
        print ""
        print "### Changed"
        print "- TODO: Summarize release changes."
      }
    }
  ' CHANGELOG.md >"$tmp"
  mv "$tmp" CHANGELOG.md
fi

if ! grep -Fq "[$version]:" CHANGELOG.md; then
  tmp=$(mktemp)
  awk -v version="$version" '
    !inserted && /^\[[0-9]+\.[0-9]+\.[0-9]+/ {
      print "[" version "]: https://github.com/kingsword09/gvc/releases/tag/v" version
      inserted = 1
    }
    { print }
    END {
      if (!inserted) {
        print "[" version "]: https://github.com/kingsword09/gvc/releases/tag/v" version
      }
    }
  ' CHANGELOG.md >"$tmp"
  mv "$tmp" CHANGELOG.md
fi

info "Refreshing Cargo.lock"
cargo metadata --format-version 1 >/dev/null

lock_version=$(awk -F'"' '
  /^\[\[package\]\]$/ { in_gvc = 0 }
  /^name = "gvc"$/ { in_gvc = 1; next }
  in_gvc && /^version = "/ { print $2; exit }
' Cargo.lock)

[[ "$lock_version" == "$version" ]] || die "Cargo.lock has gvc $lock_version, expected $version"
grep -Fq "version = \"$version\"" Cargo.toml || die "Cargo.toml was not updated"
grep -Fq "releases/download/v$version/gvc-linux-x86_64" README.md || die "README.md release URL was not updated"
grep -Fq "releases/download/v$version/gvc-linux-x86_64" README_ZH.md || die "README_ZH.md release URL was not updated"
grep -Fq "## [$version] - $release_date" CHANGELOG.md || die "CHANGELOG.md release section was not updated"
grep -Fq "[$version]: https://github.com/kingsword09/gvc/releases/tag/v$version" CHANGELOG.md || die "CHANGELOG.md release link was not updated"

if [[ $run_checks -eq 1 ]]; then
  info "Running cargo fmt check"
  cargo fmt --all -- --check

  info "Running clippy"
  cargo clippy --all-targets --all-features -- -D warnings

  info "Running tests"
  cargo test --all-features
fi

if [[ $run_package -eq 1 ]]; then
  info "Verifying crate package contents"
  package_files=$(mktemp)
  cargo package --allow-dirty --list >"$package_files"
  for packaged_file in \
    Cargo.lock \
    Cargo.toml \
    LICENSE \
    README.md \
    README_ZH.md \
    skills/gvc/SKILL.md \
    skills/gvc/agents/openai.yaml \
    src/main.rs; do
    grep -Fxq "$packaged_file" "$package_files" || die "crate package is missing $packaged_file"
  done
  rm -f "$package_files"
  cargo package --allow-dirty
fi

info "Release prep complete for v$version"
printf '\nReview before tagging:\n'
printf '  - Fill any TODO entries in CHANGELOG.md.\n'
printf '  - Review git diff.\n'
printf '  - Commit with: chore(release): prepare v%s\n' "$version"
printf '  - Tag with: git tag v%s && git push origin main v%s\n' "$version" "$version"
