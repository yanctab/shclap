# Schema Version 2 Features

Set `"schema_version": 2` in your configuration to enable these extended features.

## Environment Variable Fallback

Arguments can fall back to environment variables when not provided on the command line. Use the `env` field to specify which environment variable to check:

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

**Priority order:**
1. Command-line argument (highest)
2. Environment variable (fallback)
3. Default value (if specified)

## Multiple Values

Arguments can accept multiple values, output as bash arrays. Enable with `"multiple": true`:

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

### Delimiter Splitting

Use `delimiter` to split a single value into multiple:

```json
{"name": "tags", "long": "tags", "type": "option", "multiple": true, "delimiter": ","}
```

```bash
# --tags "one,two,three" -> SHCLAP_TAGS=("one" "two" "three")
```

### Multiple Values Per Occurrence

Use `num_args` to accept multiple values per flag occurrence:

```json
{"name": "point", "long": "point", "type": "option", "multiple": true, "num_args": "2"}
```

```bash
# --point 10 20 --point 30 40 -> SHCLAP_POINT=("10" "20" "30" "40")
```

## Subcommands

Define nested commands like `git init`, `git commit`. Each subcommand can have its own set of arguments:

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
```

### Handling Subcommands

The selected subcommand name is stored in `$SHCLAP_SUBCOMMAND`. Use a `case` statement to handle different commands:

```bash
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

### Subcommand Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `name` | string | Yes | Subcommand name |
| `help` | string | No | Help text for subcommand |
| `args` | array | No | Arguments specific to this subcommand |

## Output Format

### Arrays

Multiple-value arguments are output as bash arrays:

```bash
SHCLAP_FILES=("file1.txt" "file2.txt" "file3.txt")
```

Access elements with:
- `${SHCLAP_FILES[0]}` - First element
- `${SHCLAP_FILES[@]}` - All elements
- `${#SHCLAP_FILES[@]}` - Array length

### SHCLAP_SUBCOMMAND

When using subcommands, an additional variable is set:

```bash
SHCLAP_SUBCOMMAND="init"  # Name of the selected subcommand
```

## See Also

- [Configuration Reference](configuration.md) - Full JSON schema reference
- [Examples](examples.md) - Complete working examples
- [CLI Reference](cli-reference.md) - Command-line options
