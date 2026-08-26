# Examples

This document provides comprehensive examples of using shclap in shell scripts.

## Basic Script with Flags and Options

A simple script with verbose flag and output option:

```bash
#!/bin/bash
CONFIG='{
  "name": "process",
  "description": "Process data files",
  "args": [
    {"name": "verbose", "short": "v", "type": "flag", "help": "Enable verbose output"},
    {"name": "output", "short": "o", "type": "option", "required": true, "help": "Output file"}
  ]
}'
source $(shclap parse --config "$CONFIG" --script "$0" -- "$@")

if [[ "$SHCLAP_VERBOSE" == "true" ]]; then
  echo "Verbose mode enabled"
  echo "Writing to: $SHCLAP_OUTPUT"
fi

echo "Processing..." > "$SHCLAP_OUTPUT"
```

Usage:
```bash
./process.sh -v -o result.txt
./process.sh --verbose --output=result.txt
```

## Script with Positional Arguments

Processing input and output files:

```bash
#!/bin/bash
CONFIG='{
  "name": "convert",
  "description": "Convert file format",
  "args": [
    {"name": "input", "type": "positional", "required": true, "help": "Input file"},
    {"name": "output", "type": "positional", "required": true, "help": "Output file"},
    {"name": "format", "short": "f", "type": "option", "default": "json", "help": "Output format"}
  ]
}'
source $(shclap parse --config "$CONFIG" --script "$0" -- "$@")

echo "Converting $SHCLAP_INPUT to $SHCLAP_OUTPUT (format: $SHCLAP_FORMAT)"
```

Usage:
```bash
./convert.sh data.csv data.json
./convert.sh data.csv data.xml -f xml
```

## Environment Variable Fallback

In schema v2, arguments automatically fall back to `PREFIX + ARG_NAME` environment variables:

```bash
#!/bin/bash
CONFIG='{
  "schema_version": 2,
  "name": "api-client",
  "prefix": "API_",
  "description": "Make API requests",
  "args": [
    {"name": "host", "long": "host", "type": "option", "default": "api.example.com"},
    {"name": "key", "long": "key", "type": "option", "required": true},
    {"name": "endpoint", "type": "positional", "required": true}
  ]
}'
source $(shclap parse --config "$CONFIG" --script "$0" -- "$@")

curl -H "Authorization: Bearer $API_KEY" "https://$API_HOST/$API_ENDPOINT"
```

Usage:
```bash
# Auto-env: reads from $API_HOST and $API_KEY
export API_KEY="secret123"
./api-client.sh /users

# CLI args take precedence
./api-client.sh --host=staging.example.com --key=secret123 /users
```

### Disabling Auto-Env

Use `"env": false` to prevent reading from environment (e.g., for security):

```bash
CONFIG='{
  "schema_version": 2,
  "args": [
    {"name": "password", "type": "option", "env": false}
  ]
}'
# $SHCLAP_PASSWORD env var will NOT be read, must use --password
```

### Custom Env Var Name

Use `"env": "VAR_NAME"` for legacy or non-standard var names:

```bash
CONFIG='{
  "schema_version": 2,
  "args": [
    {"name": "api_key", "type": "option", "env": "LEGACY_API_TOKEN"}
  ]
}'
# Reads from $LEGACY_API_TOKEN instead of $SHCLAP_API_KEY
```

## Handling Multiple Values

Processing multiple files:

```bash
#!/bin/bash
CONFIG='{
  "schema_version": 2,
  "name": "batch-process",
  "description": "Process multiple files",
  "args": [
    {"name": "files", "short": "f", "long": "file", "type": "option", "multiple": true, "required": true},
    {"name": "dry_run", "short": "n", "type": "flag", "help": "Show what would be done"}
  ]
}'
source $(shclap parse --config "$CONFIG" --script "$0" -- "$@")

echo "Processing ${#SHCLAP_FILES[@]} files..."
for file in "${SHCLAP_FILES[@]}"; do
  if [[ "$SHCLAP_DRY_RUN" == "true" ]]; then
    echo "[dry-run] Would process: $file"
  else
    echo "Processing: $file"
    # actual processing here
  fi
done
```

Usage:
```bash
./batch-process.sh -f a.txt -f b.txt -f c.txt
./batch-process.sh --file=a.txt --file=b.txt -n
```

## Comma-Separated Values

Using delimiter to split values:

```bash
#!/bin/bash
CONFIG='{
  "schema_version": 2,
  "name": "tagger",
  "description": "Add tags to items",
  "args": [
    {"name": "tags", "short": "t", "type": "option", "multiple": true, "delimiter": ","},
    {"name": "item", "type": "positional", "required": true}
  ]
}'
source $(shclap parse --config "$CONFIG" --script "$0" -- "$@")

echo "Adding tags to $SHCLAP_ITEM:"
for tag in "${SHCLAP_TAGS[@]}"; do
  echo "  - $tag"
done
```

Usage:
```bash
./tagger.sh -t "bug,urgent,backend" issue-123
```

## Subcommand Pattern

A multi-command tool similar to git:

```bash
#!/bin/bash
CONFIG='{
  "schema_version": 2,
  "name": "project",
  "description": "Project management tool",
  "version": "1.0.0",
  "args": [
    {"name": "verbose", "short": "v", "type": "flag", "help": "Verbose output"}
  ],
  "subcommands": [
    {
      "name": "init",
      "help": "Initialize a new project",
      "args": [
        {"name": "name", "type": "positional", "required": true, "help": "Project name"},
        {"name": "template", "short": "t", "type": "option", "default": "basic"}
      ]
    },
    {
      "name": "build",
      "help": "Build the project",
      "args": [
        {"name": "release", "short": "r", "type": "flag", "help": "Build for release"},
        {"name": "target", "short": "t", "type": "option", "default": "default"}
      ]
    },
    {
      "name": "deploy",
      "help": "Deploy the project",
      "args": [
        {"name": "environment", "short": "e", "type": "option", "required": true},
        {"name": "force", "short": "f", "type": "flag", "help": "Force deployment"}
      ]
    }
  ]
}'
source $(shclap parse --config "$CONFIG" --script "$0" -- "$@")

# Global flag applies to all subcommands
log() {
  if [[ "$SHCLAP_VERBOSE" == "true" ]]; then
    echo "[INFO] $*"
  fi
}

case "$SHCLAP_SUBCOMMAND" in
  init)
    log "Initializing project: $SHCLAP_NAME"
    echo "Creating project '$SHCLAP_NAME' with template '$SHCLAP_TEMPLATE'"
    mkdir -p "$SHCLAP_NAME"
    ;;
  build)
    log "Starting build"
    if [[ "$SHCLAP_RELEASE" == "true" ]]; then
      echo "Building release for target: $SHCLAP_TARGET"
    else
      echo "Building debug for target: $SHCLAP_TARGET"
    fi
    ;;
  deploy)
    log "Starting deployment"
    if [[ "$SHCLAP_FORCE" == "true" ]]; then
      echo "Force deploying to $SHCLAP_ENVIRONMENT"
    else
      echo "Deploying to $SHCLAP_ENVIRONMENT"
    fi
    ;;
  *)
    echo "Unknown subcommand: $SHCLAP_SUBCOMMAND"
    exit 1
    ;;
esac
```

Usage:
```bash
./project.sh init myapp -t rust
./project.sh -v build -r
./project.sh deploy -e production -f
```

## Real-World Example: Deploy Script

A complete deployment script with multiple options:

```bash
#!/bin/bash
set -euo pipefail

CONFIG='{
  "schema_version": 2,
  "name": "deploy",
  "description": "Deploy application to servers",
  "version": "2.0.0",
  "args": [
    {"name": "environment", "short": "e", "type": "option", "required": true, "env": "DEPLOY_ENV", "help": "Target environment (staging/production)"},
    {"name": "version", "short": "V", "type": "option", "required": true, "help": "Version to deploy"},
    {"name": "servers", "short": "s", "type": "option", "multiple": true, "delimiter": ",", "help": "Target servers (comma-separated)"},
    {"name": "dry_run", "short": "n", "type": "flag", "help": "Show what would be deployed"},
    {"name": "force", "short": "f", "type": "flag", "help": "Skip confirmation prompts"},
    {"name": "notify", "type": "option", "multiple": true, "help": "Slack channels to notify"}
  ]
}'
source $(shclap parse --config "$CONFIG" --script "$0" -- "$@")

# Validate environment
if [[ "$SHCLAP_ENVIRONMENT" != "staging" && "$SHCLAP_ENVIRONMENT" != "production" ]]; then
  echo "Error: environment must be 'staging' or 'production'"
  exit 1
fi

# Set default servers if not specified
if [[ ${#SHCLAP_SERVERS[@]} -eq 0 ]]; then
  if [[ "$SHCLAP_ENVIRONMENT" == "production" ]]; then
    SHCLAP_SERVERS=("prod-1.example.com" "prod-2.example.com")
  else
    SHCLAP_SERVERS=("staging.example.com")
  fi
fi

echo "=== Deployment Plan ==="
echo "Environment: $SHCLAP_ENVIRONMENT"
echo "Version: $SHCLAP_VERSION"
echo "Servers: ${SHCLAP_SERVERS[*]}"
echo "======================="

# Confirmation for production
if [[ "$SHCLAP_ENVIRONMENT" == "production" && "$SHCLAP_FORCE" != "true" && "$SHCLAP_DRY_RUN" != "true" ]]; then
  read -p "Deploy to PRODUCTION? (yes/no): " confirm
  if [[ "$confirm" != "yes" ]]; then
    echo "Deployment cancelled"
    exit 0
  fi
fi

# Deploy to each server
for server in "${SHCLAP_SERVERS[@]}"; do
  if [[ "$SHCLAP_DRY_RUN" == "true" ]]; then
    echo "[dry-run] Would deploy v$SHCLAP_VERSION to $server"
  else
    echo "Deploying v$SHCLAP_VERSION to $server..."
    # ssh "$server" "cd /app && ./update.sh $SHCLAP_VERSION"
  fi
done

# Send notifications
if [[ ${#SHCLAP_NOTIFY[@]} -gt 0 && "$SHCLAP_DRY_RUN" != "true" ]]; then
  for channel in "${SHCLAP_NOTIFY[@]}"; do
    echo "Notifying Slack channel: $channel"
    # curl -X POST "https://slack.com/api/chat.postMessage" ...
  done
fi

echo "Deployment complete!"
```

Usage:
```bash
# Staging deployment
./deploy.sh -e staging -V 1.2.3

# Production with specific servers
./deploy.sh -e production -V 1.2.3 -s "prod-1.example.com,prod-2.example.com"

# Dry run with notifications
./deploy.sh -e production -V 1.2.3 -n --notify=#deploys --notify=#ops

# Force deployment (skip confirmation)
./deploy.sh -e production -V 1.2.3 -f
```

## Numeric Option with Double Precision Validation

Processing numeric values with floating-point precision:

```bash
#!/bin/bash
CONFIG='{
  "schema_version": 2,
  "name": "analyze",
  "description": "Analyze data with threshold filtering",
  "args": [
    {"name": "threshold", "short": "t", "long": "threshold", "type": "option", "value_type": "double", "required": true, "help": "Confidence threshold (0.0-1.0)"},
    {"name": "input", "type": "positional", "required": true, "help": "Input data file"}
  ]
}'
source $(shclap parse --config "$CONFIG" --script "$0" -- "$@")

echo "Analyzing $SHCLAP_INPUT with threshold: $SHCLAP_THRESHOLD"
```

Usage with a positive float:
```bash
./analyze.sh -t 0.95 data.csv
# $SHCLAP_THRESHOLD = "0.95"
```

Usage with a negative float:
```bash
./analyze.sh --threshold=-0.5 data.csv
# $SHCLAP_THRESHOLD = "-0.5"
```

Usage with scientific notation:
```bash
./analyze.sh -t 1e10 data.csv
# $SHCLAP_THRESHOLD = "10000000000"
```

Error from non-numeric value:
```bash
$ ./analyze.sh -t abc data.csv
shclap: invalid value 'abc' for '--threshold': invalid float literal
```

## Container Bootstrap

Automatically re-execute a script inside a container:

```bash
#!/bin/bash
set -euo pipefail

CONFIG='{
  "schema_version": 2,
  "name": "containerized-script",
  "description": "A script that runs inside a container",
  "version": "1.0.0",
  "container": {
    "runtime": "docker",
    "image": "ubuntu:22.04",
    "args": ["--network", "host"]
  },
  "args": [
    {"name": "message", "short": "m", "type": "option", "required": true, "help": "Message to display"},
    {"name": "verbose", "short": "v", "type": "flag", "help": "Verbose output"}
  ]
}'

# Guard host-only setup with SHCLAP_IN_CONTAINER check
if [[ -z "${SHCLAP_IN_CONTAINER:-}" ]]; then
  echo "Preparing on the host..."
  # Any host-specific setup here (e.g., downloading, prerequisite checks)
  # This runs only on the first (host) pass
fi

source $(shclap parse --config "$CONFIG" --script "$0" -- "$@")

# All code below runs inside the container
echo "Running inside container"
echo "Message: $SHCLAP_MESSAGE"

if [[ "$SHCLAP_VERBOSE" == "true" ]]; then
  echo "Verbose mode enabled"
  echo "Container environment:"
  env | head -5
fi
```

Usage (script automatically re-execs in container):

```bash
./containerized-script.sh -m "Hello from container"
./containerized-script.sh -m "Hello" -v
```

Note: The user just invokes the script normally. The automatic re-execution into the container happens transparently. Any code before the `source $(shclap parse ...)` line runs twice (once on the host, once in the container), so use the `SHCLAP_IN_CONTAINER` guard to prevent side effects.

## Container Pull Policy Variants

The `pull_policy` sub-field of `container` controls when the image is fetched from a registry. All three values are shown below. The field must be nested under `container`; placing it at the top level is a validation error.

### always — always pull from the registry

```bash
CONFIG='{
  "schema_version": 2,
  "name": "fresh-image",
  "container": {
    "runtime": "docker",
    "image": "ubuntu:22.04",
    "pull_policy": "always"
  },
  "args": [{"name": "verbose", "short": "v", "type": "flag"}]
}'
source $(shclap parse --config "$CONFIG" --script "$0" -- "$@")
# docker run --pull=always ... ubuntu:22.04 ...
```

Use `"always"` in CI pipelines where you want to guarantee the latest image content regardless of local cache.

### missing — pull only when not cached locally (default)

```bash
CONFIG='{
  "schema_version": 2,
  "name": "cached-image",
  "container": {
    "runtime": "docker",
    "image": "ubuntu:22.04",
    "pull_policy": "missing"
  },
  "args": [{"name": "verbose", "short": "v", "type": "flag"}]
}'
source $(shclap parse --config "$CONFIG" --script "$0" -- "$@")
# docker run --pull=missing ... ubuntu:22.04 ...
```

`"missing"` is the default when `pull_policy` is not specified. It balances freshness with performance.

### never — never pull; use local cache only

```bash
CONFIG='{
  "schema_version": 2,
  "name": "offline-image",
  "container": {
    "runtime": "docker",
    "image": "ubuntu:22.04",
    "pull_policy": "never"
  },
  "args": [{"name": "verbose", "short": "v", "type": "flag"}]
}'
source $(shclap parse --config "$CONFIG" --script "$0" -- "$@")
# docker run --pull=never ... ubuntu:22.04 ...
```

Use `"never"` in air-gapped environments or when you want a hard guarantee that no network call is made.

## Podman Runtime

Replace `"runtime": "docker"` with `"runtime": "podman"` to use Podman:

```bash
#!/bin/bash
CONFIG='{
  "schema_version": 2,
  "name": "podman-script",
  "description": "Script that bootstraps into a Podman container",
  "container": {
    "runtime": "podman",
    "image": "registry.fedoraproject.org/fedora:39",
    "pull_policy": "missing",
    "args": ["--userns=keep-id"]
  },
  "args": [
    {"name": "output", "short": "o", "type": "option", "required": true},
    {"name": "verbose", "short": "v", "type": "flag"}
  ]
}'

if [[ -z "${SHCLAP_IN_CONTAINER:-}" ]]; then
  echo "Host pass: checking prerequisites..."
fi

source $(shclap parse --config "$CONFIG" --script "$0" -- "$@")

echo "Inside Podman container"
echo "Output: $SHCLAP_OUTPUT"
```

Usage:

```bash
./podman-script.sh -o result.txt
./podman-script.sh -v -o result.txt
```

Both Docker and Podman accept `always`, `missing`, and `never` as `--pull` values, so the same `pull_policy` field works for either runtime.

## Environment Variable Forwarding

When container bootstrap re-execs into the container, shclap automatically forwards environment variables in two ways:

1. **Prefix matching**: any env var whose name starts with the configured prefix (default `SHCLAP_`) is forwarded.
2. **Explicit `env` fields**: any variable named via an argument's `env` field is forwarded.

```bash
#!/bin/bash
CONFIG='{
  "schema_version": 2,
  "name": "api-container",
  "prefix": "API_",
  "container": {
    "runtime": "docker",
    "image": "ubuntu:22.04"
  },
  "args": [
    {"name": "endpoint", "type": "positional", "required": true},
    {"name": "token", "long": "token", "type": "option", "env": "LEGACY_API_TOKEN"}
  ]
}'
source $(shclap parse --config "$CONFIG" --script "$0" -- "$@")

curl -H "Authorization: Bearer $LEGACY_API_TOKEN" "https://$API_ENDPOINT"
```

With the above config, any environment variable starting with `API_` that is set on the host is forwarded into the container via `-e API_*`. The `LEGACY_API_TOKEN` variable is also forwarded because it is named explicitly via the `env` field.

```bash
export API_BASE_URL="https://api.example.com"
export LEGACY_API_TOKEN="secret-token"
./api-container.sh /v1/users
# Inside the container: both API_BASE_URL and LEGACY_API_TOKEN are available
```

## Args With Spaces and Metacharacters

Values in `container.args` that contain shell metacharacters (spaces, semicolons, `$`, backticks, wildcards, etc.) are automatically single-quoted in the generated shell fragment so they are passed as single arguments to the runtime.

```bash
CONFIG='{
  "schema_version": 2,
  "name": "labeled-container",
  "container": {
    "runtime": "docker",
    "image": "ubuntu:22.04",
    "args": [
      "--label", "team=platform ops",
      "--label=env=prod;tier=backend",
      "--network", "host"
    ]
  },
  "args": [{"name": "verbose", "short": "v", "type": "flag"}]
}'
source $(shclap parse --config "$CONFIG" --script "$0" -- "$@")
```

The generated invocation wraps unsafe values in single quotes:

```sh
exec docker run --rm \
  --pull=missing \
  ... \
  --label \
  'team=platform ops' \
  '--label=env=prod;tier=backend' \
  --network \
  host \
  ubuntu:22.04 \
  bash "$_shclap_script" "$@"
```

Values that contain only alphanumerics and safe punctuation (`-_./:=@%+,`) are passed through unquoted for readability.

## Running an Already-Containerised Script

If the script is invoked from inside a container that was not started by shclap (for example, a CI runner or a dev shell), shclap detects this and skips re-execution:

```bash
# Inside a CI Docker container:
./myscript.sh --output=result.txt
# shclap prints to stderr: "shclap: container detected via /.dockerenv, skipping reexec"
# then proceeds with normal argument parsing
```

The detection order is:

1. `SHCLAP_IN_CONTAINER` (silent bypass — set by shclap itself)
2. `/.dockerenv` (verbose bypass — set by Docker daemon)
3. `/run/.containerenv` (verbose bypass — set by Podman/CRI-O)
4. `$container` (verbose bypass — set by Podman/systemd-nspawn)

This means the same script runs correctly whether invoked from a host machine or from inside an existing container. No configuration change is needed.

## --help and --version Interaction with Container Dispatch

Container dispatch is bypassed when `--help` or `--version` is detected in the script arguments. The help or version output is returned from the host without starting the container:

```bash
#!/bin/bash
CONFIG='{
  "schema_version": 2,
  "name": "myapp",
  "version": "1.0.0",
  "container": {
    "runtime": "docker",
    "image": "ubuntu:22.04"
  },
  "args": [
    {"name": "verbose", "short": "v", "type": "flag", "help": "Enable verbose output"},
    {"name": "input", "type": "positional", "required": true, "help": "Input file"}
  ]
}'
source $(shclap parse --config "$CONFIG" --script "$0" -- "$@")
echo "Processing: $SHCLAP_INPUT"
```

```bash
./myapp.sh --help      # Prints help text directly, no container started
./myapp.sh --version   # Prints "myapp 1.0.0", no container started
./myapp.sh input.txt   # Triggers container re-exec, then processes inside container
```

The `shclap help`, `shclap version`, and `shclap print` CLI subcommands also never perform container dispatch.

## See Also

- [Configuration Reference](configuration.md) - Full JSON schema reference
- [Container Bootstrap](container.md) - Container re-execution guide
- [Schema Reference](schema.md) - Schema versioning and v2 features
- [CLI Reference](cli-reference.md) - Command-line options
