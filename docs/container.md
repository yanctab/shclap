# Container Bootstrap

## Overview

Container bootstrap is a shclap feature that enables scripts to automatically re-execute themselves inside a container using `docker` or `podman`. When configured, shclap emits code on the host that detects whether it is already running in a container, and if not, re-execs the script inside the specified container image.

**Constraints:**

- Requires `schema_version: 2`. The feature is not available in v1.
- The `container` block must be at the top level of the configuration. It is not permitted inside a subcommand definition.
- The `shclap parse` command is the only subcommand that triggers container dispatch. The `help`, `version`, and `print` subcommands never perform re-execution.

## Bootstrap Flow

When `shclap parse` is called on a configuration that has a `container` block:

1. Arguments are parsed first (including `--help` and `--version`).
2. If the parse outcome is `--help` or `--version`, the help/version output is returned immediately and container dispatch is **skipped**.
3. Otherwise, shclap checks the container detection signals (see below).
4. If any signal is detected, re-execution is **skipped** and normal argument parsing output is returned.
5. If no signal is detected, shclap emits a shell fragment that re-execs the script inside the configured container.

## Container Detection Signals

shclap checks four signals in priority order. The first match wins.

| Priority | Signal | Type | Bypass behaviour |
|----------|--------|------|-----------------|
| 1 | `SHCLAP_IN_CONTAINER` | Environment variable | Silent pass-through (no stderr output) |
| 2 | `/.dockerenv` | Filesystem marker | Verbose pass-through (one line to stderr) |
| 3 | `/run/.containerenv` | Filesystem marker | Verbose pass-through (one line to stderr) |
| 4 | `$container` | Environment variable | Verbose pass-through (one line to stderr) |

**Signal semantics:**

- `SHCLAP_IN_CONTAINER` is set by shclap itself during bootstrap re-execution (`-e SHCLAP_IN_CONTAINER=1`). It is the authoritative signal that the script is running inside a shclap-managed container. Detection is silent — no diagnostic is printed.
- `/.dockerenv` is created by the Docker daemon inside every Docker container.
- `/run/.containerenv` is created by Podman, CRI-O, and systemd-nspawn inside containers.
- `$container` is set by Podman, systemd-nspawn, and similar runtimes.

When a non-`SHCLAP_IN_CONTAINER` signal is detected, shclap prints one diagnostic line to stderr:

```
shclap: container detected via <signal>, skipping reexec
```

where `<signal>` is one of `/.dockerenv`, `/run/.containerenv`, or `$container`.

## Re-exec Contract

When none of the detection signals are present, shclap writes a shell fragment to a temp file and prints the path. When the script sources that path, the following happens:

1. The script and shclap binary paths are resolved with `readlink -f`.
2. The configured runtime is checked with `command -v`. If it is not on `PATH`, the sourced file prints an error and exits with code **127**.
3. A diagnostic is printed to stderr: `shclap: bootstrapping into <runtime>:<image>`.
4. `exec <runtime> run` replaces the host shell process with the container.

The exact shape of the emitted invocation:

```sh
_shclap_script=$(readlink -f "$0")
_shclap_bin=$(readlink -f "$(command -v shclap)")
command -v docker >/dev/null 2>&1 || { echo "shclap: container runtime 'docker' not found" >&2; exit 127; }
echo "shclap: bootstrapping into docker:ubuntu:22.04" >&2
set -x
exec docker run --rm \
  --pull=missing \
  -v "$_shclap_script:$_shclap_script:ro" \
  -v "$_shclap_bin:/usr/local/bin/shclap:ro" \
  -e SHCLAP_IN_CONTAINER=1 \
  [forwarded env vars as -e NAME ...] \
  [container.args values ...] \
  ubuntu:22.04 \
  bash "$_shclap_script" "$@"
```

Key points:

- `--pull=<policy>` is emitted immediately after `--rm`. The value matches `container.pull_policy` verbatim (`always`, `missing`, or `never`).
- The script and shclap binary are mounted read-only into the container.
- `SHCLAP_IN_CONTAINER=1` is set so the container pass detects the bypass signal.
- All environment variables whose names start with the configured prefix are forwarded.
- All variables listed via `env` fields in the argument config are forwarded.
- Values from `container.args` are emitted as individual shell words; values that contain shell metacharacters or whitespace are single-quoted.
- The container receives the original `"$@"` arguments verbatim.
- `exec` replaces the host shell process, so there is no return from the container.

## Container Configuration Block

The `container` field is an object nested directly under the top-level config (v2 only):

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
  "args": [
    {"name": "verbose", "short": "v", "type": "flag"},
    {"name": "output", "short": "o", "type": "option"}
  ]
}
```

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `runtime` | string | Yes | Container runtime: `"docker"` or `"podman"` |
| `image` | string | Yes | Fully-qualified image reference, e.g. `registry.example.com/img:tag` |
| `pull_policy` | string | No | When to pull the image: `"always"`, `"missing"`, or `"never"`. Default: `"missing"`. |
| `args` | array of strings | No | Extra flags passed to `<runtime> run` before the image name. |

### Pull Policy

The `pull_policy` field controls when the container runtime fetches the image from a registry.

| Value | Runtime flag emitted | Behaviour |
|-------|---------------------|-----------|
| `"always"` | `--pull=always` | Always pull from the registry, even if a local copy exists. |
| `"missing"` | `--pull=missing` | Pull only if no local copy is present. **This is the default.** |
| `"never"` | `--pull=never` | Never pull; fail if the image is not cached locally. |

**The value is passed through to the runtime without translation.** Both Docker (≥ 20.10) and Podman accept `always`, `missing`, and `never` as `--pull` values.

`pull_policy` **must** be nested under `container`. Specifying `pull_policy` at the top level of the configuration is a validation error.

Any value other than `"always"`, `"missing"`, or `"never"` is rejected at parse time with an error naming the invalid value.

## Re-Execution Semantics

When container bootstrap is enabled, code **before** the `source $(shclap parse ...)` line runs twice:

1. **First execution (host):** The script runs on the local machine with the host environment.
2. **Second execution (container):** The script re-execs inside the container with the container environment.

Use the `SHCLAP_IN_CONTAINER` guard to prevent side-effecting setup from running twice:

```bash
#!/bin/bash
CONFIG='{
  "schema_version": 2,
  "name": "myscript",
  "container": {
    "runtime": "docker",
    "image": "ubuntu:22.04"
  },
  "args": [{"name": "verbose", "short": "v", "type": "flag"}]
}'

# Runs on the host only (first pass)
if [[ -z "${SHCLAP_IN_CONTAINER:-}" ]]; then
  echo "Preparing on the host..."
fi

source $(shclap parse --config "$CONFIG" -- "$@")

# Runs inside the container (second pass)
echo "Inside container. Verbose: $SHCLAP_VERBOSE"
```

## Help and Version Bypass

Passing `--help` or `--version` to the script does **not** trigger container re-execution. shclap parses those flags first and returns the help/version output directly from the host without entering the container.

```bash
./myscript.sh --help     # Returns help text, no container started
./myscript.sh --version  # Returns version text, no container started
./myscript.sh -v         # Triggers container re-exec (normal arguments)
```

The `shclap help`, `shclap version`, and `shclap print` subcommands also never perform container dispatch.

## Already-Containerised Scripts

If the script is invoked from within a container that was not started by shclap (for example, a CI container or a dev shell), shclap detects this via `/.dockerenv`, `/run/.containerenv`, or `$container` and skips re-execution. A diagnostic is printed to stderr, then normal argument parsing proceeds.

This means the same script works correctly whether it is called from a host machine or from inside an existing container.

## Signal and Stdin Note

Because `exec docker run` (or `exec podman run`) replaces the host shell process, signals and I/O behave as follows:

- **Signals:** SIGTERM and SIGINT are delivered directly to the container process.
- **Stdin/stdout/stderr:** Connected to the container process. Interactive prompts (`read`, `less`) work as expected.
- **Ctrl+C:** Terminates the container cleanly.

## See Also

- [Configuration Reference](configuration.md) - Complete `container` field specification
- [Schema Reference](schema.md) - Schema versioning and v2 features
- [Examples](examples.md) - More end-to-end examples
