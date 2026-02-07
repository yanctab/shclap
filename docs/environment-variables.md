# Environment Variables

This document covers how shclap handles environment variables, both for output and for reading fallback values.

## Output Variables

When shclap parses arguments, it outputs shell export statements that set environment variables. These variables follow a consistent naming pattern.

### Naming Pattern

```
{PREFIX}{NAME_UPPERCASE}
```

- **PREFIX**: Configurable prefix (default: `SHCLAP_`)
- **NAME_UPPERCASE**: Argument name converted to uppercase with hyphens replaced by underscores

### Examples

| Argument Name | Default Output Variable |
|---------------|-------------------------|
| `verbose` | `$SHCLAP_VERBOSE` |
| `output-file` | `$SHCLAP_OUTPUT_FILE` |
| `api-key` | `$SHCLAP_API_KEY` |
| `input` | `$SHCLAP_INPUT` |

### Custom Prefix

Override the default prefix using `--prefix` or the config `prefix` field:

```bash
# Via CLI flag
source $(shclap parse --config "$CONFIG" --prefix="APP_" -- "$@")
# Variables: $APP_VERBOSE, $APP_OUTPUT_FILE, etc.

# Via config
CONFIG='{
  "prefix": "MYAPP_",
  "args": [{"name": "verbose", "type": "flag"}]
}'
# Variable: $MYAPP_VERBOSE
```

---

## Environment Variable Fallback (Schema v2)

In schema v2, shclap can read values from environment variables when arguments are not provided on the command line. This is called "environment variable fallback" or "auto-env".

**Important:** This feature only works in schema v2. In v1, shclap never reads from environment variables.

### Priority Order

When resolving an argument's value, shclap checks sources in this order:

```
1. Command-line argument    (highest priority)
         ↓
2. Environment variable     (fallback)
         ↓
3. Default value            (if specified in config)
```

The first source that provides a value wins.

### Example Walkthrough

Consider this configuration:

```json
{
  "schema_version": 2,
  "prefix": "MYAPP_",
  "args": [
    {"name": "config", "type": "option", "default": "/etc/app.conf"}
  ]
}
```

**Scenario 1: CLI argument provided**
```bash
export MYAPP_CONFIG="/home/user/.config"
./script.sh --config="/tmp/test.conf"
# Result: $MYAPP_CONFIG = "/tmp/test.conf" (CLI wins)
```

**Scenario 2: Environment variable fallback**
```bash
export MYAPP_CONFIG="/home/user/.config"
./script.sh
# Result: $MYAPP_CONFIG = "/home/user/.config" (env fallback)
```

**Scenario 3: Default value used**
```bash
unset MYAPP_CONFIG
./script.sh
# Result: $MYAPP_CONFIG = "/etc/app.conf" (default)
```

---

## The `env` Field

The `env` field controls how an argument interacts with environment variables for fallback. It has three possible configurations:

| `env` Value | Behavior |
|-------------|----------|
| Not specified | **Auto-env**: Reads from `PREFIX + ARG_NAME` |
| `false` | **Disabled**: Never reads from environment |
| `"VAR_NAME"` | **Custom**: Reads from the specified variable |

### Auto-env (Default)

When `env` is not specified, shclap automatically checks for an environment variable named `PREFIX + ARG_NAME_UPPERCASE`:

```json
{
  "schema_version": 2,
  "prefix": "MYAPP_",
  "args": [
    {"name": "api-key", "type": "option"}
  ]
}
```

shclap will check `$MYAPP_API_KEY` if `--api-key` is not provided on the command line.

**Naming formula:**
```
PREFIX + uppercase(name with hyphens → underscores)

Example: prefix="MYAPP_", name="api-key"
         → MYAPP_ + API_KEY
         → MYAPP_API_KEY
```

### Disabling Env Fallback

Set `env` to `false` to prevent an argument from reading from any environment variable:

```json
{
  "schema_version": 2,
  "args": [
    {"name": "secret", "type": "option", "env": false}
  ]
}
```

Even if `$SHCLAP_SECRET` is set, it will not be used as a fallback.

### Custom Variable Name

Specify a string to read from a different environment variable:

```json
{
  "schema_version": 2,
  "args": [
    {"name": "config", "type": "option", "env": "LEGACY_CONFIG_PATH"}
  ]
}
```

shclap will check `$LEGACY_CONFIG_PATH` instead of `$SHCLAP_CONFIG`.

---

## Complete Example

```json
{
  "schema_version": 2,
  "name": "deploy",
  "prefix": "DEPLOY_",
  "args": [
    {"name": "target", "type": "option"},
    {"name": "token", "type": "option", "env": "AUTH_TOKEN"},
    {"name": "dry-run", "type": "flag", "env": false}
  ]
}
```

| Argument | Env Fallback Variable |
|----------|----------------------|
| `--target` | `$DEPLOY_TARGET` (auto-env) |
| `--token` | `$AUTH_TOKEN` (custom) |
| `--dry-run` | None (disabled) |

Usage:

```bash
#!/bin/bash
CONFIG='...'  # as above
source $(shclap parse --config "$CONFIG" -- "$@")

echo "Deploying to: $DEPLOY_TARGET"
echo "Using token: $DEPLOY_TOKEN"
if [[ "$DEPLOY_DRY_RUN" == "true" ]]; then
  echo "(dry run mode)"
fi
```

---

## Schema v1 vs v2

| Feature | Schema v1 | Schema v2 |
|---------|-----------|-----------|
| Output variables | Yes | Yes |
| Auto-env fallback | No | Yes |
| Custom `env` field | No | Yes |
| Disable with `env: false` | No | Yes |

To enable environment variable fallback, add `"schema_version": 2` to your configuration.

## See Also

- [Schema Reference](schema.md) - Schema versions and v2 features
- [Configuration Reference](configuration.md) - All configuration fields
- [CLI Reference](cli-reference.md) - Command-line options
