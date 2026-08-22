# Container Bootstrap Reference

This document covers how shclap detects when it's running inside a container and adjusts its bootstrap behavior accordingly.

## Overview

shclap uses a **multi-signal container detection** mechanism to determine if it's already running inside a container. When any detection signal is present, shclap skips the bootstrap reexecution process and proceeds directly to normal argument parsing.

Understanding container detection is important when:
- Your script runs inside containers (Docker, Podman, etc.)
- Your script uses shclap's container bootstrap feature to re-execute itself in a container
- You need to prevent recursive or nested container reexecutions

## Detection Signals (Priority Order)

shclap checks for container presence using four signals, evaluated in priority order. The **first signal detected** determines the result; remaining signals are not checked.

| Priority | Signal Name | Mechanism | Type | Behavior |
|----------|-------------|-----------|------|----------|
| 1 | `SHCLAP_IN_CONTAINER` | Environment variable | Silent (internal) | Set by shclap itself during bootstrap reexec; indicates shclap already performed container reexecution |
| 2 | `/.dockerenv` | File presence | Verbose (runtime) | Special file created by Docker daemon; indicates the process is running in a Docker container |
| 3 | `/run/.containerenv` | File presence | Verbose (runtime) | File created by container runtimes (systemd, cri-o, etc.); indicates the process is running in any container |
| 4 | `$container` | Environment variable | Verbose (runtime) | Set by container runtimes (systemd-nspawn, Podman, etc.); indicates the process is inside a container |

### Signal Categories

**Silent signals** (set by shclap):
- `SHCLAP_IN_CONTAINER` — Set by shclap itself when performing bootstrap reexecution
- Does not produce any stderr output
- Allows shclap to prevent infinite reexecution loops

**Verbose signals** (set by container runtime):
- `/.dockerenv`, `/run/.containerenv`, `$container` — Detected from the host container runtime
- Produce a diagnostic message on stderr when detected
- Indicate that the host runtime (Docker, Podman, systemd, etc.) placed the process in a container

## Skip Semantics

When shclap detects **any** of these four signals during startup, it:

1. Skips the bootstrap reexecution process
2. Proceeds directly to normal argument parsing with the current process
3. Does NOT attempt to reexecute itself in a container (even if configured to do so)

This skip behavior prevents:
- Infinite reexecution loops (e.g., container inside container)
- Redundant reexecution when the process is already containerized
- Nested bootstrap attempts that could fail or behave unexpectedly

### Examples

**Example 1: Script with container bootstrap config, called locally**
```bash
CONFIG='{
  "runtime": "docker",
  "image": "myapp:latest",
  ...
}'
source $(shclap parse --config "$CONFIG" -- "$@")
# No signals detected → bootstrap reexecution occurs
```

**Example 2: Same script called from inside a Docker container**
```bash
# Inside Docker container, $SHCLAP_IN_CONTAINER or /.dockerenv detected
source $(shclap parse --config "$CONFIG" -- "$@")
# Signal detected → bootstrap reexecution SKIPPED, normal parsing proceeds
```

**Example 3: Nested scenario with explicit SHCLAP_IN_CONTAINER**
```bash
# Scenario: First reexecution sets SHCLAP_IN_CONTAINER
# Any subsequent reexecution call in the same process:
# $SHCLAP_IN_CONTAINER already set → bootstrap reexecution SKIPPED
```

## Stderr Output Format

When a **non-silent signal** is detected (i.e., any of the three runtime signals), shclap emits a single diagnostic message to stderr:

```
shclap: already inside container (via <signal>) — skipping bootstrap
```

The `<signal>` placeholder is replaced with one of:
- `SHCLAP_IN_CONTAINER` (if this signal was detected; although this is internal and typically produces no output)
- `/.dockerenv` (if the Docker marker file was detected)
- `/run/.containerenv` (if the generic container marker file was detected)
- `$container` (if the container environment variable was detected)

### Output Example

```bash
$ docker run -it --rm myimage ./script.sh --help
shclap: already inside container (via /.dockerenv) — skipping bootstrap
Help text for script...
```

This message:
- Appears on **stderr only** (not stdout)
- Appears **once per invocation** when a signal is detected
- Helps users understand why bootstrap reexecution was skipped
- Does not prevent normal script execution
- Can be suppressed by redirecting stderr if desired

## Semantic Distinction: SHCLAP_IN_CONTAINER vs Runtime Signals

Understanding the difference between these signal categories is important for script design and debugging.

### SHCLAP_IN_CONTAINER (Silent, Internal)

**Set by:** shclap itself during bootstrap reexecution  
**How:** Automatically exported as an environment variable to the reexecuted process  
**Output:** No stderr message (silent)  
**Meaning:** shclap has already performed a bootstrap reexecution; the process is now executing inside the configured container  
**Use case:** Prevents infinite reexecution loops; ensures bootstrap happens exactly once per execution

**Example:**
```bash
# Local execution with bootstrap config
$ ./script.sh --flag value
# shclap detects no container signals
# → performs bootstrap reexecution
# → sets SHCLAP_IN_CONTAINER=1 in the container environment
# → subprocess runs inside the container (bootstrap complete)
# → next invocation of shclap in the container finds SHCLAP_IN_CONTAINER=1
# → skips bootstrap (already done)
```

### Runtime Signals (Verbose, Host Runtime)

**Set by:** The container runtime (Docker daemon, Podman, systemd, etc.)  
**How:** Created as filesystem markers (`/.dockerenv`, `/run/.containerenv`) or environment variables (`$container`)  
**Output:** Diagnostic stderr message when detected  
**Meaning:** The host container runtime has already placed the process in a container; the process did not reach the container via shclap's bootstrap  
**Use case:** Indicates the script is already containerized by external means; useful for debugging and diagnostics

**Example:**
```bash
# Running inside a Docker container created via docker run
$ docker run -it --rm myimage ./script.sh --flag value
# Docker runtime created /.dockerenv
# shclap detects /.dockerenv (runtime signal)
# → emits stderr message: "shclap: already inside container (via /.dockerenv) — skipping bootstrap"
# → skips bootstrap (already in container)
# → proceeds with normal parsing
```

### Decision Table

| Scenario | SHCLAP_IN_CONTAINER Set? | Runtime Signals Present? | Stderr Message? | Bootstrap Reexec? |
|----------|--------------------------|--------------------------|-----------------|-------------------|
| Local execution, no bootstrap config | No | No | No | N/A (no config) |
| Local execution, with bootstrap config | No | No | No | Yes (reexec to container) |
| Inside container via docker run | No | Yes (/.dockerenv) | Yes | No (skip, already in container) |
| Inside container via shclap bootstrap | Yes | Maybe | No | No (skip, already bootstrapped) |
| Nested: bootstrap → inside container | Yes | Yes (/.dockerenv or others) | No | No (skip, SHCLAP_IN_CONTAINER takes priority) |

## Common Questions

**Q: Why are there four signals instead of just one?**  
A: Different container runtimes use different mechanisms to mark containerized processes. shclap checks all of them to be compatible with Docker, Podman, systemd-nspawn, and other runtimes.

**Q: When should I use SHCLAP_IN_CONTAINER in my script?**  
A: You typically don't need to check it. shclap handles the detection automatically. However, you might reference it in logging or debugging to verify that bootstrap reexecution occurred.

**Q: What if my script is called from a container that itself uses shclap with bootstrap?**  
A: The outermost shclap bootstrap sets `SHCLAP_IN_CONTAINER`. Any nested shclap calls detect this signal and skip their own bootstrap, preventing infinite reexecution.

**Q: Can I disable container detection?**  
A: Not currently. Container detection is always active. If your script is already in a container and you want to bootstrap again, you would need to unset `SHCLAP_IN_CONTAINER` and clear the runtime marker files (which is not recommended).

**Q: What if none of the four signals are detected?**  
A: If no signals are found and your config specifies container bootstrap settings, shclap will perform the bootstrap reexecution.

## See Also

- [Configuration Reference](configuration.md) — Container bootstrap config fields (`runtime`, `image`, `args`)
- [CLI Reference](cli-reference.md) — shclap commands and options
- [Environment Variables](environment-variables.md) — How environment variables are managed
