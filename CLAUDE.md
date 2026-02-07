# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build Commands

```bash
make build              # Debug build
make release            # Static release binary (musl)
make fmt                # Format code
make lint               # Run clippy (fails on warnings)
make unit-test          # Run Rust unit tests
make integration-test   # Run shell integration tests
make test               # Run all tests (unit + integration)
make check              # Full check: fmt + lint + test
make deb                # Build Debian package
make clean              # Clean build artifacts
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
1. `make fmt-check`
2. `make lint`
3. `make unit-test`
4. `make release` + `make integration-test`

## Backlog Workflow

When completing a task from `BACKLOG.md`:

1. Run `make check` before committing
2. Update documentation if the feature affects user-facing behavior:
   - `docs/` - markdown documentation
   - `man/shclap.1` - man page
3. Mark the task as complete with ~~strikethrough~~ in `BACKLOG.md`
4. Commit and push
