# Configuration Reference

shclap uses JSON configuration to define your CLI interface. This document covers all available fields and options.

## Top-Level Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `schema_version` | number | No | Schema version (default: 1). Set to 2 for extended features. |
| `name` | string | No* | Name of your script/tool. *Optional if provided via CLI `--name` flag. |
| `description` | string | No | Description shown in help output |
| `version` | string | No | Version string shown with `--version` |
| `prefix` | string | No | Environment variable prefix (default: `SHCLAP_`) |
| `args` | array | No | Array of argument definitions (default: empty) |
| `subcommands` | array | No | Array of subcommand definitions (v2 only) |

**Note:** The `name` field can be omitted if you provide the application name via the CLI `--name` flag. This is useful when you want to avoid hardcoding the script name in your configuration.

## Argument Fields

Each argument in the `args` array can have the following fields:

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `name` | string | Yes | Argument name (becomes env var suffix) |
| `short` | char | No | Single character for short flag (e.g., `v` for `-v`) |
| `long` | string | No | Long flag name (defaults to `name` if no `short` specified) |
| `type` | string | Yes | One of: `flag`, `option`, `positional` |
| `required` | bool | No | Whether argument is required (default: false) |
| `default` | string | No | Default value if not provided |
| `help` | string | No | Help text shown in usage |
| `env` | string/false | No | Env fallback: omit for auto (`PREFIX+NAME`), `false` to disable, or custom var name (v2 only) |
| `multiple` | bool | No | Accept multiple values as array (v2 only) |
| `num_args` | string | No | Number of values per occurrence (v2 only) |
| `delimiter` | string | No | Split single value by delimiter (v2 only) |
| `choices` | array | No | Allowed values for this argument (v2 only) |
| `value_type` | string | No | Value type validation: "string" (default), "int", "bool" (v2 only) |

### Long Option Fallback

For non-positional arguments (`flag` or `option` types), if neither `short` nor `long` is specified, the `name` field is automatically used as the long option. This allows for concise configurations:

```json
{
  "name": "myapp",
  "args": [
    {"name": "verbose", "type": "flag"},
    {"name": "output", "type": "option"}
  ]
}
```

This is equivalent to:

```json
{
  "name": "myapp",
  "args": [
    {"name": "verbose", "long": "verbose", "type": "flag"},
    {"name": "output", "long": "output", "type": "option"}
  ]
}
```

Both configurations accept `--verbose` and `--output`.

## Argument Types

shclap supports three argument types, each with distinct behavior.

### Flag

A boolean switch that takes no value. Its presence sets the variable to `"true"`, absence to `"false"`.

**Syntax:** `-v`, `--verbose`

**Output:** `"true"` or `"false"`

```json
{"name": "verbose", "short": "v", "type": "flag"}
```

```bash
./script.sh -v           # $SHCLAP_VERBOSE = "true"
./script.sh --verbose    # $SHCLAP_VERBOSE = "true"
./script.sh              # $SHCLAP_VERBOSE = "false"
```

**With `multiple: true` (v2):** Counts occurrences instead of boolean.

```json
{"name": "verbose", "short": "v", "type": "flag", "multiple": true}
```

```bash
./script.sh -v           # $SHCLAP_VERBOSE = "1"
./script.sh -vvv         # $SHCLAP_VERBOSE = "3"
./script.sh              # $SHCLAP_VERBOSE = "0"
```

### Option

Takes a value. Can be specified with various syntaxes.

**Syntax:** `-o file`, `-ofile`, `--output file`, `--output=file`

**Output:** The provided value (or default if not specified)

```json
{"name": "output", "short": "o", "type": "option"}
```

```bash
./script.sh -o file.txt        # $SHCLAP_OUTPUT = "file.txt"
./script.sh -ofile.txt         # $SHCLAP_OUTPUT = "file.txt"
./script.sh --output file.txt  # $SHCLAP_OUTPUT = "file.txt"
./script.sh --output=file.txt  # $SHCLAP_OUTPUT = "file.txt"
```

**With `multiple: true` (v2):** Collects values as a bash array.

```json
{"name": "file", "short": "f", "type": "option", "multiple": true}
```

```bash
./script.sh -f a.txt -f b.txt  # $SHCLAP_FILE = ("a.txt" "b.txt")
```

### Positional

Identified by position, not by flags. Order matters.

**Syntax:** Values without dashes, in order

**Output:** The provided value

```json
{
  "args": [
    {"name": "input", "type": "positional"},
    {"name": "output", "type": "positional"}
  ]
}
```

```bash
./script.sh source.txt dest.txt
# $SHCLAP_INPUT = "source.txt"
# $SHCLAP_OUTPUT = "dest.txt"
```

**With `multiple: true` (v2):** The last positional collects remaining arguments as an array.

```json
{
  "args": [
    {"name": "command", "type": "positional"},
    {"name": "files", "type": "positional", "multiple": true}
  ]
}
```

```bash
./script.sh build a.txt b.txt c.txt
# $SHCLAP_COMMAND = "build"
# $SHCLAP_FILES = ("a.txt" "b.txt" "c.txt")
```

### Type Comparison

| Type | Identified By | Takes Value | Default Output | With `multiple` |
|------|--------------|-------------|----------------|-----------------|
| `flag` | `-x`, `--name` | No | `"true"`/`"false"` | Count (`"3"`) |
| `option` | `-x val`, `--name=val` | Yes | Provided value | Array |
| `positional` | Position (order) | Yes | Provided value | Array (last only) |

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
      "name": "format",
      "short": "f",
      "long": "format",
      "type": "option",
      "choices": ["json", "yaml", "toml"],
      "default": "json",
      "help": "Output format"
    },
    {
      "name": "count",
      "short": "n",
      "long": "count",
      "type": "option",
      "value_type": "int",
      "default": "10",
      "help": "Number of items to process"
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

## Minimal Example (Using Long Fallback)

A minimal configuration using the long option fallback feature:

```json
{
  "args": [
    {"name": "verbose", "type": "flag", "help": "Enable verbose output"},
    {"name": "output", "type": "option", "help": "Output file path"},
    {"name": "input", "type": "positional", "help": "Input file"}
  ]
}
```

Usage with CLI `--name` flag:

```bash
source $(shclap parse --config="$CONFIG" --name="$(basename "$0")" -- "$@")
```

This accepts: `--verbose`, `--output=file.txt`, and positional `input.txt`.

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

- [Schema Reference](schema.md) - Schema versioning and v2 features
- [Environment Variables](environment-variables.md) - Environment variable handling
- [Examples](examples.md) - Complete working examples
- [CLI Reference](cli-reference.md) - Command-line options
