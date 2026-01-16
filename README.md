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
