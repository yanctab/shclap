# CLI Reference

This document covers shclap's command-line interface and all available options.

## Commands

### `shclap parse`

Parse command-line arguments according to the JSON configuration and output a sourceable shell script.

```bash
shclap parse --config=<JSON> [--name=<NAME>] [--prefix=<PREFIX>] -- [ARGS...]
```

**Arguments:**

| Argument | Description |
|----------|-------------|
| `--config=<JSON>` | JSON configuration string (required) |
| `--name=<NAME>` | Application name (overrides config `name` field) |
| `--prefix=<PREFIX>` | Environment variable prefix (default: `SHCLAP_`) |
| `--` | Separator between shclap options and script arguments |
| `[ARGS...]` | Arguments to parse (typically `"$@"`) |

**Example:**

```bash
source $(shclap parse --config='{"args":[]}' --name=myapp -- "$@")
```

### `shclap help`

Display help information for your script (using the config).

```bash
shclap help --config=<JSON> [--name=<NAME>]
```

**Arguments:**

| Argument | Description |
|----------|-------------|
| `--config=<JSON>` | JSON configuration string (required) |
| `--name=<NAME>` | Application name (overrides config `name` field) |

**Example:**

```bash
shclap help --config='{"args":[{"name":"verbose","type":"flag"}]}' --name=myapp
```

### `shclap version`

Display version information for your script (using the config).

```bash
shclap version --config=<JSON> [--name=<NAME>]
```

**Arguments:**

| Argument | Description |
|----------|-------------|
| `--config=<JSON>` | JSON configuration string (required) |
| `--name=<NAME>` | Application name (overrides config `name` field) |

**Example:**

```bash
shclap version --config='{"version":"1.0.0"}' --name=myapp
```

## Options

### `--config=<JSON>`

**Required.** The JSON configuration defining your CLI interface.

Can be provided as:
- Inline JSON string
- Variable containing JSON

```bash
# Inline
shclap parse --config='{"name":"app","args":[]}' -- "$@"

# Variable
CONFIG='{"name":"app","args":[]}'
shclap parse --config="$CONFIG" -- "$@"
```

### `--name=<NAME>`

Override the application name from the JSON configuration. This allows you to avoid hardcoding the script name in your config.

**Priority:** CLI `--name` > config `name` field

If neither is provided, shclap will return an error.

```bash
# Name from --name flag (config doesn't need 'name' field)
shclap parse --config='{"args":[]}' --name="$0" -- "$@"

# Name from config (no --name flag needed)
shclap parse --config='{"name":"myapp","args":[]}' -- "$@"

# CLI overrides config
shclap parse --config='{"name":"ignored","args":[]}' --name="actual_name" -- "$@"
```

### `--prefix=<PREFIX>`

Override the default environment variable prefix (`SHCLAP_`).

```bash
shclap parse --config="$CONFIG" --prefix="MYAPP_" -- "$@"
# Variables become: $MYAPP_VERBOSE, $MYAPP_OUTPUT, etc.
```

## Built-in Flags

shclap automatically provides these flags for your script:

### `-h`, `--help`

Display auto-generated help message based on your configuration.

```bash
./myscript.sh --help
```

Output includes:
- Script name and description
- Usage syntax
- Available options with help text
- Available subcommands (if defined)

### `-V`, `--version`

Display version information (if `version` is set in config).

```bash
./myscript.sh --version
```

## Exit Codes

| Code | Description |
|------|-------------|
| 0 | Success |
| 1 | Invalid arguments |
| 2 | Missing required arguments |
| 3 | Invalid configuration |

## Output Format

shclap outputs shell commands that, when sourced, set environment variables:

```bash
# For simple values
export SHCLAP_VERBOSE="true"
export SHCLAP_OUTPUT="file.txt"

# For arrays (multiple values)
SHCLAP_FILES=("a.txt" "b.txt" "c.txt")

# For subcommands
export SHCLAP_SUBCOMMAND="build"
```

## Usage Patterns

### Standard Usage

```bash
#!/bin/bash
CONFIG='...'
source $(shclap parse --config "$CONFIG" -- "$@")
```

### With Error Handling

```bash
#!/bin/bash
CONFIG='...'
PARSED=$(shclap parse --config "$CONFIG" -- "$@")
if [[ $? -ne 0 ]]; then
  echo "Argument parsing failed"
  exit 1
fi
source "$PARSED"
```

### With Custom Prefix

```bash
#!/bin/bash
CONFIG='...'
source $(shclap parse --config "$CONFIG" --prefix="APP_" -- "$@")
# Use $APP_VERBOSE, $APP_OUTPUT, etc.
```

## See Also

- [Configuration Reference](configuration.md) - Full JSON schema reference
- [Schema Reference](schema.md) - Schema versioning and v2 features
- [Examples](examples.md) - Complete working examples
