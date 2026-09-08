# Collect Configuration Reference

This document covers the `shclap collect` command and its independent JSON schema for bundling and collecting files and directories.

## Overview

The `shclap collect` command gathers files and directories into named bundles according to a JSON configuration. Each bundle is a logical grouping of files with source and destination paths. You can collect all bundles or select specific ones, output to a directory or a compressed archive, and stream archives to stdout.

The collect schema is **independent** of the parse schema — they are separate features with separate schema version counters.

## Command Syntax

```bash
shclap collect --config=<JSON> [--type=<TYPE>] --out=<PATH> [--archive-format=<FORMAT>] [--bundle=<NAME>]
```

Or use a configuration file:

```bash
shclap collect --config-file=<PATH> [--type=<TYPE>] --out=<PATH> [--archive-format=<FORMAT>] [--bundle=<NAME>]
```

## Configuration Structure

A collect configuration defines named bundles, each containing an array of entry specifications:

```json
{
  "schema_version": 1,
  "bundles": {
    "backup": [
      {"from": "src/**/*.rs", "to": "source/"},
      {"from": "Cargo.toml", "to": ""}
    ],
    "docs": [
      {"from": "docs/**/*.md", "to": "docs/", "optional": true},
      {"from": "README.md", "to": "README.md"}
    ]
  }
}
```

### Top-Level Fields

#### `schema_version`

Schema version number. Optional, defaults to 1. Currently only version 1 is supported. This is an **independent** counter from the parse schema version.

#### `bundles`

A mapping of bundle names (strings) to arrays of entry specifications. Required.

Each bundle name becomes available for selection via `--bundle=<NAME>`. If no `--bundle` flag is provided, all bundles are collected.

## Entry Specifications

Each entry in a bundle array defines how to collect a single set of files.

### Entry Fields

#### `from`

**Required.** The source glob pattern or file path. Supports:

- Literal file paths: `"README.md"`, `"src/main.rs"`
- Directory paths: `"docs/"` (all files recursively)
- Glob patterns: `"src/**/*.rs"` (all `.rs` files under `src/`)
- Single-level wildcards: `"src/*.txt"` (only `.txt` in `src/`, not subdirs)

All paths are relative to the current working directory at the time `shclap collect` is invoked.

#### `to`

**Required.** The destination path within the output bundle. Supports:

- Empty string `""` for placing files at the bundle root
- Directory paths: `"source/"` (files placed in `source/` subdirectory)
- Specific file paths: `"README.md"` (rename to a specific name)

When `from` matches multiple files (via glob), `to` is treated as a directory prefix and the relative structure is preserved. For example:
- `from: "src/**/*.rs"`, `to: "source/"` → files are placed as `source/src/main.rs`, `source/src/lib.rs`, etc.
- `from: "README.md"`, `to: "README.md"` → the file is placed exactly at `README.md` in the bundle root

#### `optional`

**Optional.** Boolean, default `false`. If `false` (default), the entry is required and collection fails if no files match the `from` pattern. If `true`, missing files are silently skipped.

## Glob Semantics

Glob patterns follow standard shell glob conventions:

- `*` matches any sequence of characters except `/` (single directory level)
- `**` matches any sequence of characters including `/` (recursive across directories)
- `?` matches any single character except `/`
- `[abc]` matches any character in the set
- `[!abc]` matches any character not in the set
- `\` escapes the next character (to match literal special chars)

Examples:
- `src/**/*.rs` matches all `.rs` files anywhere under `src/`
- `*.txt` matches all `.txt` files in the current directory only
- `docs/**/index.md` matches all `index.md` files anywhere under `docs/`
- `src/*/main.rs` matches `main.rs` only in direct subdirs of `src/` (not deeply nested)

## Environment Variable Expansion

Configuration fields can reference environment variables using bash-style syntax. Variable expansion occurs at parse time against the host environment.

Supported syntax:
- `$NAME` — expands to the value of environment variable `NAME`
- `${NAME}` — expands to the value of environment variable `NAME` (supports names with special characters)
- `$$` — expands to a literal `$` character

Expansion applies to:
- `bundles` keys (bundle names)
- `from` and `to` fields in entries
- Top-level fields like `schema_version` (if they contain variables)

If a referenced variable does not exist in the host environment, shclap will produce an error message during collection.

Example:

```bash
export SRC_DIR="/home/user/project/src"
export BACKUP_DIR="/backups"

CONFIG='{
  "schema_version": 1,
  "bundles": {
    "backup": [
      {"from": "$SRC_DIR/**/*.rs", "to": "source/"},
      {"from": "${BACKUP_DIR}/metadata.txt", "to": "meta.txt"}
    ]
  }
}'
shclap collect --config "$CONFIG" --type=dir --out=backup
```

## File Collision Behavior

When multiple entries reference the same destination path within a bundle:

- By default (collision error): Collection fails with an error message listing the conflicting entries.
- With `optional: true` on conflicting entries: Those entries are skipped if a collision would occur; later non-optional entries override.
- Last writer wins: If multiple non-optional entries target the same destination, the last entry in the bundle definition takes precedence (earlier files are overwritten).

## Output Types

### Directory Output (`--type=dir`)

Collects files into a directory structure. The `--out=<PATH>` argument specifies the output directory.

- If the directory does not exist, it is created (including parent directories).
- If the directory exists, files are merged into it.
- File collisions follow the collision behavior rules above.

Example:

```bash
shclap collect --config "$CONFIG" --type=dir --out=/tmp/bundle
```

### Archive Output (`--type=archive`)

Collects files into a compressed archive. The `--out=<PATH>` argument specifies the archive file path.

The archive format is auto-detected from the file extension. Both the long and
the conventional short forms are recognised:

| Format | Extensions |
| --- | --- |
| `tar` | `.tar` |
| `tar.gz` | `.tar.gz`, `.tgz` |
| `tar.bz2` | `.tar.bz2`, `.tbz2`, `.tbz` |
| `tar.xz` | `.tar.xz`, `.txz` |
| `zip` | `.zip` |

If the extension is ambiguous or missing, use `--archive-format` to specify the format explicitly.

Supported formats:
- `tar` — uncompressed TAR archive
- `tar.gz` — gzip-compressed TAR
- `tar.bz2` — bzip2-compressed TAR
- `tar.xz` — xz-compressed TAR
- `zip` — ZIP archive

Example:

```bash
shclap collect --config "$CONFIG" --type=archive --out=bundle.tar.gz
shclap collect --config "$CONFIG" --type=archive --out=backup.zip
```

### Streaming to stdout (`--out=-`)

When collecting to an archive, use `--out=-` to write the archive to stdout instead of a file. This allows piping to other tools or redirecting to a file with custom names.

The `--archive-format` flag **must** be specified when using `--out=-` (format cannot be auto-detected from a filename).

With `--out=-`, stdout carries the archive bytes and nothing else — the
destination is reported on stderr instead, so the stream stays byte-exact and
can be piped straight into `gzip`, `tar`, or `ssh`. For every other `--out`
value, stdout is the output path as usual.

Example:

```bash
# Stream archive to stdout and capture in a variable
ARCHIVE=$(shclap collect --config "$CONFIG" --type=archive --out=- --archive-format=tar.gz)

# Pipe to another command
shclap collect --config "$CONFIG" --type=archive --out=- --archive-format=zip | ssh user@host 'cat > /tmp/backup.zip'

# Redirect to a file
shclap collect --config "$CONFIG" --type=archive --out=- --archive-format=tar.bz2 > backup.tar.bz2
```

## Bundle Selection

By default, `shclap collect` collects **all** bundles defined in the configuration into the output.

Use `--bundle=<NAME>` to collect only a specific bundle:

```bash
shclap collect --config "$CONFIG" --type=dir --out=output --bundle=backup
```

This collects only the `backup` bundle; other bundles are ignored. If the specified bundle does not exist, collection fails with an error.

## Logging and Diagnostics

Collect respects the `SHCLAP_LOG` environment variable to control log output:

```bash
export SHCLAP_LOG=debug
shclap collect --config "$CONFIG" --type=dir --out=output
```

Log levels:
- `trace` — Very detailed debugging
- `debug` — Detailed progress (which bundles, which files)
- `info` (default) — Important milestones
- `warn` — Warnings only
- `error` — Errors only
- `off` — No logging

Log output is written to stderr; the actual collected files go to the specified `--out` location.

## Path Safety and Normalization

Paths are canonicalized and validated to prevent path traversal attacks:

- Relative paths are resolved against the current working directory.
- Symlinks are resolved to their targets.
- Paths containing `..` segments are rejected if they escape the bundle root.
- On Windows, backslashes are normalized to forward slashes.

This ensures that malicious or accidental path specifications cannot write files outside the intended bundle location.

## Exit Status

- **0** — Successful collection
- **1** — Collection failed (missing required entries, path errors, invalid configuration, etc.)
- **2** — Invalid arguments or missing required flags

## Error Messages

### Missing Required Entries

If an entry with `optional: false` (default) does not match any files:

```
shclap: collection failed: entry at line X did not match any files: src/missing/**/*.rs
```

### File Collision

If two entries target the same destination and collision resolution fails:

```
shclap: collection failed: file collision at 'source/config.toml' from multiple entries
```

### Invalid Archive Format

If an archive format is not supported:

```
shclap: unknown archive format: .tar.7z (supported: tar, tar.gz, tar.bz2, tar.xz, zip)
```

### Missing --archive-format with --out -

If using `--out=-` without specifying format:

```
shclap: --archive-format is required when using --out=- with archive type
```

## Container-Transparent Guarantee

The `shclap collect` command works transparently inside containers:

- When invoked inside a container (detected via `SHCLAP_IN_CONTAINER`, `/.dockerenv`, `/run/.containerenv`, or `$container`), collection proceeds normally within the container's filesystem.
- Paths are resolved relative to the container's working directory.
- Glob patterns work against container paths.
- The collected output is available at the specified `--out` location (or stdout) from within the container.

This makes `shclap collect` suitable for use in containerized environments without special handling.

## See Also

- [CLI Reference](cli-reference.md) — `shclap collect` command-line syntax
- [Schema Reference](schema.md) — Collect schema versioning (note: independent from parse schema)
- [Configuration Reference](configuration.md) — Parse schema (separate from collect schema)
