# shclap

Declarative argument parsing for shell scripts. Define your CLI in JSON, get parsed arguments as environment variables.

## Why shclap?

Writing argument parsing in shell scripts is tedious and error-prone. Manual `getopts` or `getopt` code is verbose, hard to read, and doesn't give you nice error messages or auto-generated help.

**shclap** lets you:
- **Declare your CLI in JSON** instead of writing imperative parsing code
- **Get auto-generated `--help` and `--version`** for free
- **Validate arguments** with clear error messages
- **Access parsed values as environment variables** with a customizable prefix

## Quick Start

```bash
#!/bin/bash
CONFIG='{
  "name": "greet",
  "args": [
    {"name": "name", "type": "positional", "required": true}
  ]
}'
source $(shclap parse --config "$CONFIG" -- "$@")
echo "Hello, $SHCLAP_NAME!"
```

## How It Works

1. **Define** your arguments in JSON (inline or in a file)
2. **Parse** by running `shclap parse` with your config and script arguments
3. **Source** the output file to get environment variables

All parsed arguments become environment variables with a prefix (default: `SHCLAP_`). For example:
- `--output file.txt` → `$SHCLAP_OUTPUT="file.txt"`
- `--verbose` → `$SHCLAP_VERBOSE="true"`
- positional `input` → `$SHCLAP_INPUT="value"`

The prefix is customizable via `--prefix` or in your config.

## Features

### Schema Versions

shclap supports two schema versions:

**v1** (default) — Simple scripts:
- Flags (`-v`, `--verbose`)
- Options (`-o file`, `--output=file`)
- Positional arguments

**v2** — Advanced use cases:
- Environment variable fallback for options
- Multiple values as bash arrays
- Subcommands

### Environment Variable Output

All parsed arguments are exported as environment variables:
- Default prefix: `SHCLAP_`
- Customize with `--prefix` flag or `prefix` in config
- Names are uppercased: `--my-option` → `$SHCLAP_MY_OPTION`

## Installation

### From GitHub Releases

Download from [Releases](https://github.com/yanctab/shclap/releases):
- `shclap-x.x.x-x86_64-linux-musl` - Static binary
- `shclap_x.x.x_amd64.deb` - Debian package

### From Source

```bash
make setup-build-env && make release && make install
```

## Documentation

- [Configuration Reference](docs/configuration.md)
- [Schema Reference](docs/schema.md)
- [Examples](docs/examples.md) — More complex use cases
- [CLI Reference](docs/cli-reference.md)

## Development

```bash
make check    # fmt + lint + test
make release  # Build static binary
make deb      # Build .deb package
```

## License

Apache-2.0
