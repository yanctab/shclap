# Logging

This document covers shclap's logging features and the shell helper functions available when using `shclap parse`.

## Shell Helper Functions

When you source the output of `shclap parse`, five logging helper functions are automatically defined in your shell session. These functions provide convenient access to leveled logging output:

- `log_trace` — Output a trace-level message
- `log_debug` — Output a debug-level message
- `log_info` — Output an info-level message
- `log_warn` — Output a warn-level message
- `log_error` — Output an error-level message

Each function accepts a message as arguments and delegates to the `shclap log` subcommand. Messages are written to stderr with a level prefix (e.g., `INFO:`, `ERROR:`).

### Using the Helper Functions

After sourcing the output of `shclap parse`, you can call any of the five helpers:

```bash
#!/bin/bash
CONFIG='{"name":"myapp","args":[]}'
source $(shclap parse --config "$CONFIG" -- "$@")

# Use the helper functions
log_info "Application started"
log_debug "Verbose debugging enabled"
log_warn "This is a warning"
log_error "An error occurred"
log_trace "Low-level trace information"
```

### Shadowing Helper Functions

You can override the emitted helper functions by declaring your own after sourcing:

```bash
#!/bin/bash
source $(shclap parse --config "$CONFIG" -- "$@")

# Define your own log_info
log_info() {
    echo "[CUSTOM] $*" >&2
}

log_info "Now uses custom implementation"
```

## Logging Control Variables

### SHCLAP_LOG

The `SHCLAP_LOG` environment variable controls the minimum log level displayed by the logging helper functions and the `shclap log` subcommand.

**Valid values:** `trace`, `debug`, `info` (default), `warn`, `error`, `off`

When set, only messages at or above the specified level are output:

```bash
# Show only warnings and errors
export SHCLAP_LOG=warn
log_info "Not displayed"      # Below threshold
log_warn "Displayed"           # At threshold
log_error "Displayed"          # Above threshold

# Silence all logging
export SHCLAP_LOG=off
log_info "Not displayed"       # Silenced
```

**Default behavior:** If `SHCLAP_LOG` is not set, the default level is `info`.

### SHCLAP_LOG_STYLE

The `SHCLAP_LOG_STYLE` environment variable controls whether log output includes ANSI color codes.

**Valid values:** `auto` (default), `always`, `never`

- `auto` — Use color only if stderr is connected to a terminal (TTY)
- `always` — Always use color codes
- `never` — Never use color codes

```bash
# Force colored output
export SHCLAP_LOG_STYLE=always
log_info "Colored message"

# Disable color for piping to files
export SHCLAP_LOG_STYLE=never
log_info "Plain text message" > logfile.txt
```

## The `shclap log` Subcommand

You can also call `shclap log` directly without using the helper functions:

```bash
shclap log info "Direct logging call"
shclap log warn "Something might be wrong"
shclap log error "Error: operation failed"
```

The subcommand respects both `SHCLAP_LOG` and `SHCLAP_LOG_STYLE` environment variables.

## Output Format

Log messages are output to stderr with the following format:

```
LEVEL: message
```

Where `LEVEL` is the uppercase log level name (with `warn` displayed as `WARNING`). When color is enabled, the level is displayed in the corresponding ANSI color:

- `TRACE` — Cyan
- `DEBUG` — Magenta
- `INFO` — Green
- `WARN` / `WARNING` — Yellow
- `ERROR` — Red

Example output (with colors disabled):

```
INFO: Application started
DEBUG: Configuration loaded from /etc/app.conf
WARN: Deprecated option used
ERROR: Failed to connect to database
```

## Performance Considerations

### Fork-per-Call Cost

Each call to a logging helper function (or `shclap log` directly) creates a new subprocess. For scripts that log frequently or in tight loops, this fork overhead should be considered:

```bash
# This creates 1,000 subprocesses
for i in {1..1000}; do
    log_debug "Processing item $i"
done
```

For high-frequency logging or performance-critical sections, consider batching log messages or using a custom logging implementation that doesn't fork for each call.

If logging performance is a concern:
1. Use a higher `SHCLAP_LOG` level to reduce output (e.g., set to `error` in production)
2. Implement custom batched logging
3. Consider writing directly to your own log file instead of using the helpers

## Backward Compatibility

The logging helper functions are only emitted when using `shclap parse`. If you use other subcommands (`help`, `version`, `print`), or if error occurs during parsing, the helper functions are not available.

Scripts written before logging was introduced will continue to work without modification. The helper functions are optional—your script can ignore them and use custom logging instead.

## See Also

- [CLI Reference](cli-reference.md) - The `shclap log` subcommand
- [Environment Variables](environment-variables.md) - Environment variable configuration
