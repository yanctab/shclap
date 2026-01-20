# shclap

Clap-style argument parsing for shell scripts.

## Overview

shclap brings the power of [clap](https://docs.rs/clap/)-style argument parsing to shell scripts. Define your CLI interface via JSON config, and shclap validates arguments and outputs a sourceable file with environment variables.

## Installation

### From .deb package (Ubuntu/Debian)

```bash
sudo dpkg -i shclap_0.1.0_amd64.deb
```

### From source

```bash
make setup-build-env
make release
make install
```

## Usage

```bash
#!/bin/bash
CONFIG='{
  "name": "myscript",
  "description": "My awesome script",
  "args": [
    {"name": "verbose", "short": "v", "type": "flag", "help": "Enable verbose output"},
    {"name": "output", "short": "o", "type": "option", "required": true, "help": "Output file"},
    {"name": "input", "type": "positional", "help": "Input file to process"}
  ]
}'
source $(shclap --config="$CONFIG" -- "$@")

# Now available:
# $SHCLAP_VERBOSE = "true" or "false"
# $SHCLAP_OUTPUT = "value"
# $SHCLAP_INPUT = "value"
```

## JSON Config Schema

```json
{
  "schema_version": "number (optional, default 1)",
  "name": "string (required)",
  "description": "string (optional)",
  "version": "string (optional)",
  "args": [
    {
      "name": "string (required) - becomes env var name",
      "short": "char (optional) - single char like 'v'",
      "long": "string (optional) - defaults to name",
      "type": "flag|option|positional (required)",
      "required": "bool (optional, default false)",
      "default": "string (optional)",
      "help": "string (optional)"
    }
  ]
}
```

## Schema Version 2 Features

Set `"schema_version": 2` to enable extended features.

### Environment Variable Fallback

Arguments can fall back to environment variables when not provided on the command line:

```bash
CONFIG='{
  "schema_version": 2,
  "name": "myapp",
  "args": [
    {"name": "api_key", "long": "api-key", "type": "option", "env": "MYAPP_API_KEY"}
  ]
}'
source $(shclap parse --config "$CONFIG" -- "$@")
# $SHCLAP_API_KEY comes from --api-key or $MYAPP_API_KEY
```

### Multiple Values

Arguments can accept multiple values, output as bash arrays:

```bash
CONFIG='{
  "schema_version": 2,
  "name": "myapp",
  "args": [
    {"name": "files", "long": "file", "type": "option", "multiple": true}
  ]
}'
source $(shclap parse --config "$CONFIG" -- --file a.txt --file b.txt)
# $SHCLAP_FILES is a bash array: ("a.txt" "b.txt")
for f in "${SHCLAP_FILES[@]}"; do
  echo "Processing $f"
done
```

Use `delimiter` to split a single value:

```bash
{"name": "tags", "long": "tags", "type": "option", "multiple": true, "delimiter": ","}
# --tags "one,two,three" -> SHCLAP_TAGS=("one" "two" "three")
```

Use `num_args` to accept multiple values per occurrence:

```bash
{"name": "point", "long": "point", "type": "option", "multiple": true, "num_args": "2"}
# --point 10 20 --point 30 40 -> SHCLAP_POINT=("10" "20" "30" "40")
```

### Subcommands

Define nested commands like `git init`, `git commit`:

```bash
#!/bin/bash
CONFIG='{
  "schema_version": 2,
  "name": "myapp",
  "args": [
    {"name": "verbose", "short": "v", "type": "flag"}
  ],
  "subcommands": [
    {
      "name": "init",
      "help": "Initialize a new project",
      "args": [
        {"name": "template", "type": "positional", "default": "default"}
      ]
    },
    {
      "name": "build",
      "help": "Build the project",
      "args": [
        {"name": "release", "short": "r", "type": "flag", "help": "Release build"}
      ]
    }
  ]
}'
source $(shclap parse --config "$CONFIG" -- "$@")

# $SHCLAP_SUBCOMMAND contains the subcommand name
case "$SHCLAP_SUBCOMMAND" in
  init)
    echo "Initializing with template: $SHCLAP_TEMPLATE"
    ;;
  build)
    if [[ "$SHCLAP_RELEASE" == "true" ]]; then
      echo "Building release..."
    else
      echo "Building debug..."
    fi
    ;;
esac
```

## Argument Types

| Type | Description | Example |
|------|-------------|---------|
| `flag` | Boolean switch | `-v`, `--verbose` |
| `option` | Takes a value | `-o file`, `--output=file` |
| `positional` | Positional argument | `input.txt` |

## Options

| Option | Description |
|--------|-------------|
| `--config=<json>` | JSON configuration (required) |
| `--prefix=<PREFIX>` | Environment variable prefix (default: `SHCLAP_`) |
| `--help` | Show help message |
| `--version` | Show version |

## Development

```bash
# Set up build environment
make setup-build-env

# Build
make build

# Run tests
make test

# Format and lint
make check

# Build release binary (static, musl)
make release

# Build .deb package
make deb
```

## License

Apache-2.0
