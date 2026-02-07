# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build Commands

```bash
# Development
cargo build                    # Debug build
cargo fmt                      # Format code
cargo clippy -- -D warnings    # Lint (fails on warnings)
cargo test                     # Unit tests

# Full check (format + lint + all tests)
make check

# Integration tests (requires build first)
./tests/integration.sh

# Release build (static binary with musl)
make release                   # or: cargo build --release --target x86_64-unknown-linux-musl

# Debian package
make deb
```

## Running a Single Test

```bash
cargo test test_name           # Run specific unit test
cargo test -- --nocapture      # Show println! output
```

## Architecture

shclap is a CLI tool that enables declarative argument parsing for shell scripts. Users define their CLI in JSON, and shclap outputs a file of shell export statements that the script sources.

### Core Flow

1. `main.rs` - Entry point using clap. Handles `parse`, `help`, and `version` subcommands
2. `config.rs` - JSON schema parsing and validation. Supports schema v1 (basic) and v2 (env fallback, multi-value, subcommands)
3. `parser.rs` - Argument parsing logic. Returns `ParseOutcome` (Success, Help, Version, or Error)
4. `output.rs` - Generates shell export statements written to a temp file
5. `help.rs` - Generates help text

### Schema Versions

- **v1**: Flags, options, positional args
- **v2**: Adds `env` (environment variable fallback), `multiple` (array values), `delimiter`, `num_args`, and subcommands

### Output Mechanism

shclap writes export statements to a temp file and prints the path. Shell scripts source this:
```bash
source $(shclap parse --config "$CONFIG" -- "$@")
```

Parsed values become environment variables with configurable prefix (default: `SHCLAP_`).

## CI

GitHub Actions runs on PRs to main:
1. `cargo fmt --check`
2. `cargo clippy -- -D warnings`
3. `cargo test`
4. `./tests/integration.sh`
