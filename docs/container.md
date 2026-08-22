# Container Bootstrap

## Overview

Container bootstrap is a shclap feature that enables scripts to automatically re-execute themselves inside a container using `docker` or `podman`. When configured, shclap emits code on the host that detects whether it's already running in a container, and if not, re-execs the script inside the specified container image. This allows you to author portable scripts that transparently run their logic in a containerized environment.

## Configuration

The `container` node is a top-level configuration object in schema v2 that specifies container runtime parameters. When present, shclap generates a re-exec wrapper at the top of your sourced output.

```json
{
  "schema_version": 2,
  "name": "myscript",
  "container": {
    "runtime": "docker",
    "image": "artifactory.example.com/team/img:v1.2.3",
    "args": ["--network", "host"]
  },
  "args": [
    {"name": "verbose", "short": "v", "type": "flag"},
    {"name": "output", "short": "o", "type": "option"}
  ]
}
```

See [Configuration Reference](configuration.md) for the `container` field specification.

## Container Image Pull Policy

The `pull_policy` field controls when container images are pulled from a registry. This is useful for controlling whether shclap always fetches the latest version of an image, or reuses a locally cached image if available.

**Valid values:**

- `"always"` — Always pull the image from the registry, even if a local copy exists. Ensures the latest version is used.
- `"never"` — Never pull; use only a locally cached image. If the image is not present locally, the container fails to start.
- `"if-not-present"` — (Default) Pull only if the image is not already cached locally. Balances freshness with performance.

**Example with pull_policy:**

```json
{
  "schema_version": 2,
  "name": "myscript",
  "container": {
    "runtime": "docker",
    "image": "artifactory.example.com/team/img:v1.2.3",
    "pull_policy": "always",
    "args": ["--network", "host"]
  },
  "args": [
    {"name": "verbose", "short": "v", "type": "flag"},
    {"name": "output", "short": "o", "type": "option"}
  ]
}
```

When `pull_policy` is set, shclap passes the `--pull` flag to the container runtime with the specified policy value. For example, `"pull_policy": "always"` emits `docker run --pull always ...` in the generated shell code.

## Emitted Shell File

When `container` is configured, shclap writes code before the normal argument parsing export statements. Here's an annotated example of what the emitted file contains:

```sh
# Path to the running script (may be a symlink—readlink -f resolves it)
_shclap_script=$(readlink -f "$0")

# Path to the shclap binary itself
_shclap_bin=$(readlink -f "$(command -v shclap)")

# Check that the runtime is available
command -v docker >/dev/null 2>&1 || { echo "shclap: container runtime 'docker' not found" >&2; exit 127; }

# Replace the current shell with a docker container, mounting the script and shclap binary
exec docker run --rm \
  -v "$_shclap_script:$_shclap_script:ro" \
  -v "$_shclap_bin:/usr/local/bin/shclap:ro" \
  -e SHCLAP_IN_CONTAINER=1 \
  -e SHCLAP_FOO=bar \
  --network host \
  artifactory.example.com/team/img:v1.2.3 \
  bash "$_shclap_script" "$@"
```

Key points:
- **Script mounting**: The script itself and the shclap binary are mounted as read-only volumes.
- **Environment marker**: `SHCLAP_IN_CONTAINER=1` is set inside the container to prevent re-execution loops.
- **Argument passing**: `"$@"` passes all original script arguments to the container.
- **Extra args**: Arguments from the `container.args` field (e.g., `--network host`) are inserted before the image name. Each value is emitted as one shell word and single-quoted when it contains anything the shell would act on, so a value such as `"my label"` reaches the runtime as a single argument rather than two.
- **Exit replacement**: `exec` replaces the host shell process with the container, so the container becomes the script's process.

## Re-Execution Semantics Warning

When container bootstrap is enabled, **code before the `source $(shclap parse ...)` line runs twice**:

1. **First execution (host):** The script runs on your local machine with the host's environment.
2. **Second execution (container):** The script re-exececs inside the container with the container's environment.

Side-effecting setup code (downloads, file creation, environment setup) must be guarded to run only once:

```bash
#!/bin/bash
CONFIG='{
  "schema_version": 2,
  "name": "myscript",
  "container": {
    "runtime": "docker",
    "image": "ubuntu:latest"
  },
  "args": [...]
}'

# This runs on the host only (first execution)
if [[ -z "${SHCLAP_IN_CONTAINER:-}" ]]; then
  echo "Preparing on the host..."
  # Download, compile, setup, etc.
fi

source $(shclap parse --config "$CONFIG" -- "$@")

# This runs inside the container
echo "Running in container: $SHCLAP_VERBOSE"
```

The `SHCLAP_IN_CONTAINER` variable is set to `"1"` on the container re-execution, allowing you to conditionally guard host-only code.

## Signal and Stdin Note

When shclap's emitted code uses `exec docker run`, it replaces the host shell process. This has two important consequences:

- **Signals:** Signals (SIGTERM, SIGINT, etc.) are delivered to the container process, not the host shell. The container handles them directly.
- **Stdin/Stdout:** The script runs with the container as its process, so stdin/stdout/stderr are connected directly to the container.

In practice, this means users can Ctrl+C the script as expected, and it cleanly terminates the container.

## Multi-Subcommand Example

Here's a complete example using subcommands where each subcommand runs inside the same container:

```bash
#!/bin/bash
set -euo pipefail

CONFIG='{
  "schema_version": 2,
  "name": "jm",
  "description": "Job manager tool",
  "version": "1.0.0",
  "container": {
    "runtime": "docker",
    "image": "ubuntu:latest",
    "args": ["--network", "host"]
  },
  "args": [
    {"name": "verbose", "short": "v", "type": "flag", "help": "Verbose output"}
  ],
  "subcommands": [
    {
      "name": "list",
      "help": "List all jobs",
      "args": [
        {"name": "status", "short": "s", "type": "option", "default": "all", "help": "Filter by status (all, running, failed)"}
      ]
    },
    {
      "name": "run",
      "help": "Run a job",
      "args": [
        {"name": "job_name", "type": "positional", "required": true, "help": "Name of the job to run"},
        {"name": "timeout", "short": "t", "type": "option", "value_type": "int", "default": "3600", "help": "Timeout in seconds"}
      ]
    },
    {
      "name": "stop",
      "help": "Stop a running job",
      "args": [
        {"name": "job_id", "type": "positional", "required": true, "help": "ID of the job to stop"}
      ]
    }
  ]
}'

# Only run setup on the host (first pass)
if [[ -z "${SHCLAP_IN_CONTAINER:-}" ]]; then
  echo "Verifying prerequisites..."
  command -v docker >/dev/null || { echo "Error: docker not found"; exit 127; }
fi

source $(shclap parse --config "$CONFIG" -- "$@")

# Utility function that respects verbose flag
log() {
  if [[ "$SHCLAP_VERBOSE" == "true" ]]; then
    echo "[INFO] $*"
  fi
}

case "$SHCLAP_SUBCOMMAND" in
  list)
    log "Listing jobs (status: $SHCLAP_STATUS)"
    # In the container, query the job database
    echo "Running jobs: job-001, job-002"
    echo "Failed jobs: job-099"
    ;;
  run)
    log "Running job: $SHCLAP_JOB_NAME (timeout: ${SHCLAP_TIMEOUT}s)"
    # In the container, start the job
    echo "Job $SHCLAP_JOB_NAME started"
    sleep 2
    echo "Job completed successfully"
    ;;
  stop)
    log "Stopping job: $SHCLAP_JOB_ID"
    # In the container, stop the job
    echo "Job $SHCLAP_JOB_ID stopped"
    ;;
  *)
    echo "Unknown subcommand: $SHCLAP_SUBCOMMAND"
    exit 1
    ;;
esac
```

Usage:

```bash
# List all jobs (re-executes in container automatically)
./jm.sh list

# List running jobs only
./jm.sh list -s running

# Run a job with custom timeout
./jm.sh run my-task -t 1800

# Stop a job
./jm.sh stop job-001

# Enable verbose output for any subcommand
./jm.sh -v list -s running
```

## See Also

- [Configuration Reference](configuration.md) - Complete `container` field specification
- [Schema Reference](schema.md) - Schema versioning and v2 features
- [Examples](examples.md) - More end-to-end examples
