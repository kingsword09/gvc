# Contributing to GVC

Thank you for your interest in contributing to GVC! This document provides guidelines and instructions for contributing.

## Getting Started

1. **Fork the repository** on GitHub
2. **Clone your fork** locally:
   ```bash
   git clone https://github.com/YOUR_USERNAME/gvc.git
   cd gvc
   ```
3. **Add the upstream remote**:
   ```bash
   git remote add upstream https://github.com/kingsword09/gvc.git
   ```

## Development Setup

### Prerequisites

- Rust 1.85 or later (install via [rustup](https://rustup.rs/))
- Git

### Building

```bash
# Debug build
cargo build

# Release build
cargo build --release
```

### Running Tests

```bash
# Run all tests
cargo test

# Run tests with output
cargo test -- --nocapture

# Run specific test
cargo test test_name
```

### Code Quality

Before submitting a PR, ensure your code passes all checks:

```bash
# Format code
cargo fmt

# Run linter
cargo clippy --all-targets --all-features -- -D warnings

# Or use the Makefile
make check
```

## Making Changes

### Branch Naming

- Feature: `feat/description`
- Bug fix: `fix/description`
- Documentation: `docs/description`
- Refactor: `refactor/description`

Example: `feat/add-async-http-support`

### Commit Messages

Follow the [Conventional Commits](https://www.conventionalcommits.org/) specification:

```
<type>(<scope>): <subject>

<body>

<footer>
```

**Types:**
- `feat`: New feature
- `fix`: Bug fix
- `docs`: Documentation changes
- `style`: Code style changes (formatting, etc.)
- `refactor`: Code refactoring
- `test`: Adding or updating tests
- `chore`: Maintenance tasks

**Examples:**
```
feat(maven): add async HTTP client for concurrent queries

Implemented async Maven repository queries using tokio and 
reqwest async client. This reduces check time from 4.5s to 0.5s
for projects with 25+ dependencies.

Closes #123
```

```
fix(version): correctly handle SNAPSHOT versions

SNAPSHOT versions were incorrectly considered stable.
Updated the stability check to filter them out.

Fixes #456
```

### Code Style

- Follow Rust standard style guidelines
- Use `cargo fmt` to format code
- Address all `cargo clippy` warnings
- Add documentation comments for public APIs
- Keep functions focused and reasonably sized
- Write meaningful variable names

### Testing

- Add tests for new features
- Update tests when modifying existing features
- Ensure all tests pass before submitting PR
- Aim for good test coverage

Example test:
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_comparison() {
        let v1 = Version::parse("1.0.0").unwrap();
        let v2 = Version::parse("2.0.0").unwrap();
        assert!(v1 < v2);
    }
}
```

## Submitting Changes

1. **Update your fork** with the latest upstream changes:
   ```bash
   git fetch upstream
   git rebase upstream/main
   ```

2. **Push your changes**:
   ```bash
   git push origin your-branch-name
   ```

3. **Create a Pull Request** on GitHub

4. **Fill out the PR template** with:
   - Description of changes
   - Related issue numbers
   - Testing performed
   - Screenshots (if applicable)

## Pull Request Guidelines

- Keep PRs focused on a single feature or fix
- Update documentation if needed
- Add tests for new functionality
- Ensure CI passes
- Respond to review feedback promptly
- Keep commits clean and meaningful

## Code Review Process

1. Maintainers will review your PR
2. Address any requested changes
3. Once approved, your PR will be merged
4. Your contribution will be included in the next release

## Need Help?

- Check existing [issues](https://github.com/kingsword09/gvc/issues)
- Create a new issue for bugs or feature requests
- Join discussions in existing issues

## License

By contributing to GVC, you agree that your contributions will be licensed under the Apache-2.0 License.

Thank you for contributing! 🎉
