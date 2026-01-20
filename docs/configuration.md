# Configuration Reference

shclap uses JSON configuration to define your CLI interface. This document covers all available fields and options.

## Top-Level Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `schema_version` | number | No | Schema version (default: 1). Set to 2 for extended features. |
| `name` | string | Yes | Name of your script/tool |
| `description` | string | No | Description shown in help output |
| `version` | string | No | Version string shown with `--version` |
| `prefix` | string | No | Environment variable prefix (default: `SHCLAP_`) |
| `args` | array | Yes | Array of argument definitions |
| `subcommands` | array | No | Array of subcommand definitions (v2 only) |

## Argument Fields

Each argument in the `args` array can have the following fields:

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `name` | string | Yes | Argument name (becomes env var suffix) |
| `short` | char | No | Single character for short flag (e.g., `v` for `-v`) |
| `long` | string | No | Long flag name (defaults to `name`) |
| `type` | string | Yes | One of: `flag`, `option`, `positional` |
| `required` | bool | No | Whether argument is required (default: false) |
| `default` | string | No | Default value if not provided |
| `help` | string | No | Help text shown in usage |
| `env` | string | No | Environment variable fallback (v2 only) |
| `multiple` | bool | No | Accept multiple values as array (v2 only) |
| `num_args` | string | No | Number of values per occurrence (v2 only) |
| `delimiter` | string | No | Split single value by delimiter (v2 only) |

## Argument Types

| Type | Description | Example | Output |
|------|-------------|---------|--------|
| `flag` | Boolean switch, no value | `-v`, `--verbose` | `"true"` or `"false"` |
| `option` | Takes a value | `-o file`, `--output=file` | The provided value |
| `positional` | Positional argument | `input.txt` | The provided value |

## Complete Example

```json
{
  "schema_version": 2,
  "name": "myapp",
  "description": "My awesome application",
  "version": "1.0.0",
  "args": [
    {
      "name": "verbose",
      "short": "v",
      "type": "flag",
      "help": "Enable verbose output"
    },
    {
      "name": "output",
      "short": "o",
      "long": "output",
      "type": "option",
      "required": true,
      "help": "Output file path"
    },
    {
      "name": "config",
      "short": "c",
      "type": "option",
      "default": "config.json",
      "env": "MYAPP_CONFIG",
      "help": "Configuration file"
    },
    {
      "name": "input",
      "type": "positional",
      "help": "Input file to process"
    }
  ]
}
```

## Environment Variable Output

Arguments are converted to environment variables using this pattern:

```
{PREFIX}{NAME_UPPERCASE}
```

For example, with default prefix `SHCLAP_`:
- `name: "verbose"` -> `$SHCLAP_VERBOSE`
- `name: "output_file"` -> `$SHCLAP_OUTPUT_FILE`
- `name: "api-key"` -> `$SHCLAP_API_KEY` (hyphens become underscores)

## See Also

- [Schema Version 2 Features](schema-v2.md) - Extended features like env fallback, multiple values, subcommands
- [Examples](examples.md) - Complete working examples
- [CLI Reference](cli-reference.md) - Command-line options
