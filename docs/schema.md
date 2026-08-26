# Schema Reference

This document covers shclap's schema versioning system and the features available in each version.

## Overview

shclap uses a schema version number to enable backwards-compatible feature additions. The `schema_version` field in your configuration determines which features are available:

- **Version 1** (default): Basic flags, options, and positional arguments
- **Version 2**: Adds environment variable fallback, multiple values, and subcommands

See [Configuration Reference](configuration.md) for the complete field reference.

## Choosing a Schema Version

| Use Case | Recommended Version |
|----------|---------------------|
| Simple scripts with basic flags and options | v1 (default) |
| Need environment variable fallback | v2 |
| Need multiple values (arrays) | v2 |
| Need value type validation (int, bool, double) | v2 |
| Need subcommands like `git init`, `git commit` | v2 |
| Need container bootstrap (auto re-exec into docker/podman) | v2 |

## Schema Version 1 (Default)

Version 1 is the default and requires no explicit `schema_version` field. It supports:

- **Flags**: Boolean switches (`-v`, `--verbose`)
- **Options**: Key-value arguments (`-o file`, `--output=file`)
- **Positional arguments**: Unnamed arguments (`input.txt`)
- **Default values**: Fallback when argument not provided
- **Required arguments**: Validation for mandatory arguments
- **Auto-generated help**: `--help` and `--version` flags

### Example

```bash
#!/bin/bash
CONFIG='{
  "name": "process",
  "description": "Process data files",
  "args": [
    {"name": "verbose", "short": "v", "type": "flag"},
    {"name": "output", "short": "o", "type": "option", "required": true},
    {"name": "input", "type": "positional"}
  ]
}'
source $(shclap parse --config "$CONFIG" --script "$0" -- "$@")
```

### Limitations

Version 1 does not support:
- Environment variable fallback (`env` field)
- Multiple values (`multiple` field)
- Value choices/enums (`choices` field)
- Value type validation (`value_type` field)
- Subcommands (`subcommands` field)

## Schema Version 2

Enable version 2 by adding `"schema_version": 2` to your configuration:

```json
{
  "schema_version": 2,
  "name": "myapp",
  "args": [...]
}
```

### Environment Variable Fallback

Schema v2 enables automatic fallback to environment variables when arguments aren't provided on the command line.

**Priority order:**

```
CLI argument  >  Environment variable  >  Default value
  (highest)         (fallback)            (lowest)
```

**Quick example:**

```bash
CONFIG='{
  "schema_version": 2,
  "prefix": "MYAPP_",
  "args": [
    {"name": "config", "type": "option", "default": "/etc/app.conf"}
  ]
}'

# Scenario 1: CLI wins
export MYAPP_CONFIG="/home/override"
./script.sh --config="/tmp/test"    # Result: /tmp/test

# Scenario 2: Env fallback
export MYAPP_CONFIG="/home/override"
./script.sh                          # Result: /home/override

# Scenario 3: Default used
unset MYAPP_CONFIG
./script.sh                          # Result: /etc/app.conf
```

**Auto-env naming:** `PREFIX` + `ARG_NAME` (uppercased, hyphens become underscores)

Example: `prefix="APP_"`, `name="api-key"` → checks `$APP_API_KEY`

**Controlling fallback with the `env` field:**

| `env` Value | Behavior |
|-------------|----------|
| Not specified | Auto: reads `PREFIX + ARG_NAME` |
| `false` | Disabled: never reads from env |
| `"VAR_NAME"` | Custom: reads specified variable |

For complete documentation, see [Environment Variables](environment-variables.md).

### Multiple Values

Arguments can accept multiple values, output as bash arrays. Enable with `"multiple": true`:

```bash
CONFIG='{
  "schema_version": 2,
  "name": "myapp",
  "args": [
    {"name": "files", "long": "file", "type": "option", "multiple": true}
  ]
}'
source $(shclap parse --config "$CONFIG" --script "$0" -- --file a.txt --file b.txt)
# $SHCLAP_FILES is a bash array: ("a.txt" "b.txt")
for f in "${SHCLAP_FILES[@]}"; do
  echo "Processing $f"
done
```

#### Delimiter Splitting

Use `delimiter` to split a single value into multiple:

```json
{"name": "tags", "long": "tags", "type": "option", "multiple": true, "delimiter": ","}
```

```bash
# --tags "one,two,three" -> SHCLAP_TAGS=("one" "two" "three")
```

#### Multiple Values Per Occurrence

Use `num_args` to accept multiple values per flag occurrence:

```json
{"name": "point", "long": "point", "type": "option", "multiple": true, "num_args": "2"}
```

```bash
# --point 10 20 --point 30 40 -> SHCLAP_POINT=("10" "20" "30" "40")
```

### Value Choices (Enums)

Restrict an argument to a set of valid values using the `choices` field. Invalid values will be rejected with a clear error message:

```json
{"name": "format", "long": "format", "type": "option", "choices": ["json", "yaml", "toml"]}
```

```bash
$ myapp --format json   # OK
$ myapp --format xml    # Error: invalid value 'xml' for '--format'
```

Choices work with both options and positional arguments:

```json
{"name": "action", "type": "positional", "choices": ["start", "stop", "restart"]}
```

**Notes:**
- Choices cannot be used with flags (flags are boolean and don't accept values)
- The choices array must have at least one value
- Duplicate values in choices are not allowed
- Valid values are shown in help output

### Value Type Validation

Validate that argument values match expected types using the `value_type` field. Invalid values will be rejected with clear error messages.

```json
{"name": "count", "long": "count", "type": "option", "value_type": "int"}
{"name": "enabled", "long": "enabled", "type": "option", "value_type": "bool"}
{"name": "port", "type": "positional", "value_type": "int"}
```

**Supported types:**

| Type | Description | Valid Examples | Invalid Examples |
|------|-------------|----------------|------------------|
| `string` | Any string value (default) | Any value | N/A |
| `int` | Signed 64-bit integer | `42`, `-10`, `0` | `abc`, `3.14` |
| `bool` | Strict boolean | `true`, `false` | `yes`, `no`, `1`, `0` |
| `double` | IEEE 754 64-bit float | `3.14`, `-2.7`, `0`, `1e10` | `abc`, `3.x` |

```bash
$ myapp --count 42        # OK
$ myapp --count abc       # Error: invalid digit found in string
$ myapp --count -10       # OK (negatives allowed for int)
$ myapp --enabled true    # OK
$ myapp --enabled yes     # Error: invalid value 'yes'
```

**Notes:**
- `value_type` cannot be used with flags (flags are inherently boolean by presence/absence)
- If not specified, defaults to `string` (no validation)
- If both `choices` and `value_type` are specified, `choices` takes precedence (it's more restrictive)
- `bool` uses strict `true`/`false` only—not `yes`/`no` or `1`/`0`

### Subcommands

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
source $(shclap parse --config "$CONFIG" --script "$0" -- "$@")
```

#### Handling Subcommands

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

#### Subcommand Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `name` | string | Yes | Subcommand name |
| `help` | string | No | Help text for subcommand |
| `args` | array | No | Arguments specific to this subcommand |

### Container Bootstrap

Container bootstrap enables scripts to automatically re-execute themselves inside a container. When configured, shclap emits code that detects whether the script is already running in a container and, if not, re-execs the script inside the specified container image using `docker` or `podman`.

**Constraints:**

- Schema v2 only; not supported in v1.
- The `container` block is not valid inside a subcommand definition; it is a top-level feature only.
- The `shclap help`, `shclap version`, and `shclap print` subcommands never trigger container dispatch. When `--help` or `--version` is passed to the script, container dispatch is also bypassed.

**Example configuration:**

```json
{
  "schema_version": 2,
  "name": "myscript",
  "container": {
    "runtime": "docker",
    "image": "ubuntu:22.04",
    "pull_policy": "missing",
    "args": ["--network", "host"]
  },
  "args": [...]
}
```

See [Container Bootstrap](container.md) for complete documentation and examples.

#### Pull Policy

The `pull_policy` sub-field of `container` controls when the image is fetched from a registry:

| Value | Behaviour |
|-------|-----------|
| `"always"` | Always pull from the registry, even if a local copy exists. |
| `"missing"` | Pull only if no local copy is present. **Default.** |
| `"never"` | Never pull; use only the locally cached image. |

`pull_policy` **must** be nested under `container`. Placing it at the top level of the configuration is a validation error. Any value other than `"always"`, `"missing"`, or `"never"` is rejected at parse time.

The value is passed to the runtime as `--pull=<value>` without translation; both Docker (≥ 20.10) and Podman accept all three values.

#### Container Detection Signals

shclap checks four signals in priority order to determine whether re-execution should be skipped:

| Priority | Signal | Bypass output |
|----------|--------|---------------|
| 1 | `SHCLAP_IN_CONTAINER` (env var) | Silent — no stderr output |
| 2 | `/.dockerenv` (file) | One line to stderr |
| 3 | `/run/.containerenv` (file) | One line to stderr |
| 4 | `$container` (env var) | One line to stderr |

`SHCLAP_IN_CONTAINER` is set by shclap itself during bootstrap (`-e SHCLAP_IN_CONTAINER=1`). It is the authoritative marker that the script is running inside a shclap-managed container. Its detection is always silent.

The other three signals are set by the host container runtime (Docker daemon, Podman, systemd-nspawn, CRI-O) before shclap runs. When detected, shclap prints one line to stderr and then proceeds to normal argument parsing:

```
shclap: container detected via <signal>, skipping reexec
```

#### Emitted Re-exec Contract

When no detection signal is found, shclap emits a shell fragment with this shape:

```sh
_shclap_script=/home/user/myscript.sh
_shclap_bin=/usr/local/bin/shclap
_shclap_cwd=/home/user/project
command -v docker >/dev/null 2>&1 || { echo "shclap: container runtime 'docker' not found" >&2; exit 127; }
echo "shclap: bootstrapping into docker:ubuntu:22.04" >&2
set -x
exec docker run --rm \
  --pull=missing \
  -v "$_shclap_script:$_shclap_script:ro" \
  -v "$_shclap_bin:/usr/local/bin/shclap:ro" \
  -v "$_shclap_cwd:$_shclap_cwd" \
  -e SHCLAP_IN_CONTAINER=1 \
  --workdir "$_shclap_cwd" \
  [forwarded env vars as -e NAME ...] \
  [container.args values ...] \
  ubuntu:22.04 \
  bash "$_shclap_script" "$@"
```

Key guarantees:

- The three variables `_shclap_script`, `_shclap_bin`, and `_shclap_cwd` are emitted as shell-quoted literal values, resolved at parse time by shclap (not dynamically by the shell). All symlinks are resolved to their physical paths.
- `--pull=<policy>` appears immediately after `--rm`; the value matches `pull_policy` verbatim.
- If the runtime binary is not on `PATH`, the sourced file exits with code **127**.
- All environment variables whose names start with the configured prefix are forwarded.
- `container.args` values are emitted as individual shell words; metacharacter-containing values are single-quoted.
- `exec` replaces the host process — there is no return from the container.

#### Help, Version, and Print Bypass

Container dispatch only fires on a successful parse outcome. The following always bypass container re-execution:

- `--help` anywhere in the script arguments
- `--version` anywhere in the script arguments (and `--help` is not present)
- The `shclap help`, `shclap version`, and `shclap print` subcommands

### Output Format

#### Arrays

Multiple-value arguments are output as bash arrays:

```bash
SHCLAP_FILES=("file1.txt" "file2.txt" "file3.txt")
```

Access elements with:
- `${SHCLAP_FILES[0]}` - First element
- `${SHCLAP_FILES[@]}` - All elements
- `${#SHCLAP_FILES[@]}` - Array length

#### SHCLAP_SUBCOMMAND

When using subcommands, an additional variable is set:

```bash
SHCLAP_SUBCOMMAND="init"  # Name of the selected subcommand
```

## Migration from v1 to v2

Migrating from version 1 to version 2 is straightforward:

1. Add `"schema_version": 2` to your configuration
2. All existing v1 configurations work unchanged in v2

```json
{
  "schema_version": 2,
  "name": "myapp",
  "args": [...]
}
```

Version 2 is fully backwards-compatible with version 1 configurations.

## See Also

- [Configuration Reference](configuration.md) - Full JSON schema reference
- [Container Bootstrap](container.md) - Container re-execution guide
- [Environment Variables](environment-variables.md) - Environment variable handling
- [Examples](examples.md) - Complete working examples
- [CLI Reference](cli-reference.md) - Command-line options
