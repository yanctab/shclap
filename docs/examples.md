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
source $(shclap parse --config "$CONFIG" -- "$@")

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
source $(shclap parse --config "$CONFIG" -- "$@")

echo "Converting $SHCLAP_INPUT to $SHCLAP_OUTPUT (format: $SHCLAP_FORMAT)"
```

Usage:
```bash
./convert.sh data.csv data.json
./convert.sh data.csv data.xml -f xml
```

## Environment Variable Fallback

Using environment variables for sensitive data:

```bash
#!/bin/bash
CONFIG='{
  "schema_version": 2,
  "name": "api-client",
  "description": "Make API requests",
  "args": [
    {"name": "api_key", "long": "api-key", "type": "option", "env": "API_KEY", "required": true},
    {"name": "endpoint", "type": "positional", "required": true}
  ]
}'
source $(shclap parse --config "$CONFIG" -- "$@")

curl -H "Authorization: Bearer $SHCLAP_API_KEY" "https://api.example.com/$SHCLAP_ENDPOINT"
```

Usage:
```bash
# Using environment variable
export API_KEY="secret123"
./api-client.sh /users

# Using command-line argument
./api-client.sh --api-key=secret123 /users
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
source $(shclap parse --config "$CONFIG" -- "$@")

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
source $(shclap parse --config "$CONFIG" -- "$@")

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
source $(shclap parse --config "$CONFIG" -- "$@")

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
source $(shclap parse --config "$CONFIG" -- "$@")

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

## See Also

- [Configuration Reference](configuration.md) - Full JSON schema reference
- [Schema Version 2 Features](schema-v2.md) - Extended features
- [CLI Reference](cli-reference.md) - Command-line options
