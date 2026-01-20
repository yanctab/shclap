# shclap

Clap-style argument parsing for shell scripts.

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

## Installation

### From GitHub Releases

Download from [Releases](https://github.com/yanctab/shclap/releases):
- `shclap-x.x.x-x86_64-linux-musl` - Static binary
- `shclap_x.x.x_amd64.deb` - Debian package

### From Source

```bash
make setup-build-env && make release && make install
```

## Features

- **Flags**: `-v`, `--verbose`
- **Options**: `-o file`, `--output=file`
- **Positional arguments**: `input.txt`
- **Environment variable fallback** (v2)
- **Multiple values as bash arrays** (v2)
- **Subcommands** (v2)

## Documentation

- [Configuration Reference](docs/configuration.md)
- [Schema Reference](docs/schema.md)
- [Examples](docs/examples.md)
- [CLI Reference](docs/cli-reference.md)

## Development

```bash
make check    # fmt + lint + test
make release  # Build static binary
make deb      # Build .deb package
```

## License

Apache-2.0
