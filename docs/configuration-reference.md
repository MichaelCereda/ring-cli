# Configuration Reference

This document describes the complete YAML schema for ring-cli configuration files. Learn how to structure commands, flags, and variables to create powerful CLI tools.

## Overview

Ring-cli configurations are YAML files (version 2.0) that define a named group of commands. Each command can have flags, execute shell operations, or nest further subcommands. Configurations support placeholder substitution for runtime values and environment variables.

## Configuration Format

A minimal configuration:

```yaml
version: "2.0"
name: "deploy"
description: "Deployment operations"
commands:
  staging:
    description: "Deploy to staging"
    flags: []
    cmd:
      run:
        - "echo Deploying to staging"
```

## Configuration Fields

Top-level fields in a configuration file:

| Field | Required | Type | Description |
|-------|----------|------|-------------|
| `version` | Yes | string | Config format version. Must be `"2.0"`. No backward compatibility with 1.x. |
| `name` | Yes | string | Configuration name. Becomes a top-level subcommand when multiple configs are composed. Must be unique per alias. |
| `description` | Yes | string | Human-readable description shown in `--help` output. |
| `base-dir` | No | string | Working directory for command execution. Relative paths resolve from the config file's location. Absolute paths are used as-is. |
| `banner` | No | string | Text displayed on stderr when the alias is invoked. Useful for warnings or version info. Hidden with `-q` (quiet mode). |
| `commands` | Yes | object | Map of command names to command definitions. At least one command must exist. |

### Config Validation Rules

- `version` must be exactly `"2.0"`
- `name` must not be empty; only alphanumerics, hyphens, and underscores allowed
- `description` should be non-empty and user-friendly
- `commands` must contain at least one command
- All command names must be unique within the config

## Commands

Each command can be a leaf operation (with flags and actions) or a container (with subcommands).

### Command Fields

| Field | Required | Type | Description |
|---|---|---|---|
| `description` | Yes | string | Description shown in `--help`. |
| `flags` | No | array | List of flags this command accepts. Empty array or omit if no flags. |
| `cmd` | * | object | Action to execute (`run`). Required if no `subcommands`. Mutually exclusive with `subcommands`. |
| `subcommands` | * | object | Map of nested subcommands. Required if no `cmd`. Mutually exclusive with `cmd`. |

* A command must have either `cmd` or `subcommands`, but not both.

### Command Validation

Ring-cli validates every command recursively:

```yaml
commands:
  deploy:
    description: "Deployment"
    subcommands:
      staging:
        description: "Deploy to staging"
        # ERROR: has neither cmd nor subcommands
        flags: []
```

This config is invalid because `staging` doesn't define `cmd` or `subcommands`.

### Leaf Commands

A leaf command executes an action:

```yaml
commands:
  greet:
    description: "Greet someone"
    flags:
      - name: "name"
        short: "n"
        description: "Person to greet"
    cmd:
      run:
        - "echo Hello, ${{name}}!"
```

Usage: `myalias greet --name Alice`

### Container Commands

A container command has only subcommands:

```yaml
commands:
  cloud:
    description: "Cloud operations"
    flags: []          # optional; usually empty for containers
    subcommands:
      aws:
        description: "AWS operations"
        subcommands:
          deploy:
            description: "Deploy to AWS"
            flags: []
            cmd:
              run:
                - "echo Deploying to AWS..."
```

Usage: `myalias cloud aws deploy`

### Nesting Depth

Subcommands can be nested arbitrarily deep. The only practical limit is usability:

```yaml
commands:
  level1:
    subcommands:
      level2:
        subcommands:
          level3:
            subcommands:
              level4:
                description: "Very deep"
                flags: []
                cmd:
                  run:
                    - "echo Deeply nested"
```

## Flags

Flags are named parameters that commands accept. They become CLI arguments like `--name` or `-n`.

### Flag Fields

| Field | Required | Type | Description |
|---|---|---|---|
| `name` | Yes | string | Flag name (used as `--name`). Must be non-empty. Hyphens and underscores allowed. |
| `short` | No | string | Single-character short form (used as `-n`). Must be a single character if provided. |
| `description` | Yes | string | Description shown in `--help`. Should be concise. |

### Flag Examples

#### Long flag only

```yaml
flags:
  - name: "output"
    description: "Output file path"
```

Usage: `myalias command --output result.txt`

#### With short form

```yaml
flags:
  - name: "verbose"
    short: "v"
    description: "Enable verbose output"
```

Usage: `myalias command -v` or `myalias command --verbose`

#### Multiple flags

```yaml
flags:
  - name: "branch"
    short: "b"
    description: "Git branch"
  - name: "force"
    short: "f"
    description: "Force operation"
  - name: "message"
    description: "Commit message"
```

Usage: `myalias command -b main -f --message "Release v1.0"`

### Flag Naming Conventions

- Use lowercase with hyphens: `--my-flag` (not `--myFlag` or `--MY_FLAG`)
- Short forms are single characters: `-v`, `-o`, `-f` (not `-verb` or `-output`)
- Avoid reserved names: `--help`, `--version`, `--quiet`, `--verbose`, `--color`

## Commands: CmdType (run)

The `cmd` field specifies actions to execute. Currently, ring-cli supports shell command execution via `run`.

### Run: Execute Shell Commands

```yaml
cmd:
  run:
    - "command 1"
    - "command 2"
    - "command 3"
```

Commands in the `run` list execute sequentially. If any command fails (non-zero exit code), execution stops immediately.

#### Single Command

```yaml
commands:
  backup:
    description: "Backup the database"
    flags: []
    cmd:
      run:
        - "mysqldump -u root mydb > backup.sql"
```

#### Multiple Commands

```yaml
commands:
  deploy:
    description: "Full deployment"
    flags:
      - name: "env"
        description: "Target environment"
    cmd:
      run:
        - "echo Deploying to ${{env}}"
        - "git pull origin main"
        - "npm install"
        - "npm run build"
        - "systemctl restart app"
```

If `git pull` fails, the remaining steps don't run.

#### Complex Shell Scripts

Multi-line commands use shell syntax:

```yaml
commands:
  process:
    description: "Process files"
    flags: []
    cmd:
      run:
        - |
          for file in /data/*.txt; do
            echo "Processing $file"
            wc -l "$file"
          done
        - "echo All done"
```

The `|` (literal block scalar) preserves newlines. The shell interprets the entire block as a single command.

## Variable Substitution

Ring-cli supports two types of placeholders for runtime values and environment variables.

### Flag Placeholders

Reference flag values using `${{flag_name}}`:

```yaml
commands:
  deploy:
    description: "Deploy to target"
    flags:
      - name: "target"
        short: "t"
        description: "Deployment target"
    cmd:
      run:
        - "curl -X POST https://deploy.example.com/${{target}}/start"
```

Usage:

```bash
myalias deploy --target production
# Expands to: curl -X POST https://deploy.example.com/production/start
```

### Environment Variable Placeholders

Reference environment variables using `${{env.VAR_NAME}}`:

```yaml
commands:
  deploy:
    description: "Deploy with authentication"
    flags:
      - name: "version"
        description: "Version to deploy"
    cmd:
      run:
        - "curl -H 'Authorization: Bearer ${{env.API_TOKEN}}' -d 'version=${{version}}' https://api.example.com/deploy"
```

Before running, set the environment variable:

```bash
export API_TOKEN="secret-token-123"
myalias deploy --version 2.5
```

### Placeholder Syntax

- Flag placeholder: `${{flag_name}}` (flag name as written in the config, not the long form)
- Environment variable: `${{env.VAR_NAME}}` (case-sensitive)
- Both are substituted before shell execution
- If a flag is not provided but is referenced in the command, ring-cli errors with a clear message
- If an environment variable is not set, the placeholder is passed as-is to the shell (which may cause errors)

### Escaping Placeholders

To use literal `${{` in a command, escape it as `$${{`:

```yaml
cmd:
  run:
    - "echo 'Template: ${{{{template}}}}')"  # outputs: Template: {{template}}
```

(This is rarely needed; escape only if your command actually uses the `${{` syntax.)

## Multiple Configurations Per Alias

An alias can combine multiple configuration files. Each config's `name` field becomes a top-level subcommand.

### Via Command Line

```bash
ring-cli init --alias ops --config-path deploy.yml --config-path db.yml --config-path monitoring.yml
```

Usage:

```bash
ops deploy staging           # from deploy.yml (name: "deploy")
ops db migrate               # from db.yml (name: "db")
ops monitoring status        # from monitoring.yml (name: "monitoring")
```

### Name Conflict Handling

If two configs use the same `name`, init fails:

```bash
ring-cli init --alias ops --config-path api-v1.yml --config-path api-v2.yml
# Error: Config name conflict: "api" (from api-v1.yml and api-v2.yml)
```

**Solutions:**

1. Rename one config's `name` field
2. Use `--warn-only-on-conflict` to downgrade the error to a warning (last config wins):

```bash
ring-cli init --alias ops --config-path api-v1.yml --config-path api-v2.yml --warn-only-on-conflict
```

## References File Format

Instead of listing configs individually, create a references file that lists them:

```bash
ring-cli init --alias ops --references .ring-cli/references.yml
```

### References File Schema

```yaml
banner: "Welcome to Ops CLI"  # optional; overrides per-config banners
configs:
  - deploy.yml
  - db.yml
  - monitoring.yml
```

### Path Resolution

Paths in the references file are resolved relative to the file's own location:

```
.ring-cli/
  references.yml        # This file
  deploy.yml            # .ring-cli/deploy.yml
  schemas/
    db.yml              # .ring-cli/schemas/db.yml
```

In `references.yml`:

```yaml
configs:
  - deploy.yml           # resolves to .ring-cli/deploy.yml
  - schemas/db.yml       # resolves to .ring-cli/schemas/db.yml
```

### OpenAPI Entries

References files can also include OpenAPI specs (see [OpenAPI Support Guide](openapi-guide.md)):

```yaml
banner: "Infrastructure CLI"
configs:
  - deploy.yml
  - openapi:./api-spec.yml
  - openapi:https://api.example.com/spec.json
```

## Working Directory (base-dir)

The `base-dir` field sets the working directory for command execution.

### Absolute Path

```yaml
version: "2.0"
name: "project"
description: "Project commands"
base-dir: "/home/alice/myproject"
commands:
  build:
    description: "Build the project"
    flags: []
    cmd:
      run:
        - "make"     # Runs in /home/alice/myproject
```

### Relative Path

Relative to the config file's location:

```
configs/
  deploy.yml          # This file
  scripts/
    build.sh
  ../shared.sh
```

In `configs/deploy.yml`:

```yaml
base-dir: "../"          # Parent directory of configs/
commands:
  build:
    description: "Build"
    flags: []
    cmd:
      run:
        - "./build.sh"   # Runs in the parent directory
```

### No base-dir

If omitted, commands run in the current working directory (where the alias is invoked):

```yaml
version: "2.0"
name: "tools"
description: "General tools"
# No base-dir — runs in current directory
commands:
  whoami:
    description: "Print current user"
    flags: []
    cmd:
      run:
        - "whoami"
```

## Banner

The `banner` field displays a message when the alias is invoked. Banners are printed to stderr so they don't interfere with piped output.

### Per-Config Banner

```yaml
version: "2.0"
name: "deploy"
description: "Deployment tools"
banner: "Deploy CLI v2.0 — use with caution in production"
commands: ...
```

When invoked, the banner prints before the command runs:

```bash
$ myalias deploy staging
Deploy CLI v2.0 — use with caution in production
<command output>
```

### Top-Level Banner

When using multiple configs, a references file can define a top-level banner that takes priority:

```yaml
banner: "Welcome to Infrastructure CLI — Production Ready"
configs:
  - deploy.yml
  - db.yml
  - monitoring.yml
```

Top-level banners override per-config banners.

### Suppress Banners

Use quiet mode to hide banners:

```bash
myalias deploy staging -q
# No banner printed
```

## Complete Examples

### Example 1: Simple Deploy Tool

`deploy.yml`:

```yaml
version: "2.0"
name: "deploy"
description: "Deployment operations"
banner: "Deploy CLI v1.0"
base-dir: "../"
commands:
  staging:
    description: "Deploy to staging"
    flags:
      - name: "branch"
        short: "b"
        description: "Git branch to deploy"
      - name: "skip-tests"
        description: "Skip running tests"
    cmd:
      run:
        - |
          if [ -z "$skip_tests" ]; then
            npm test || exit 1
          fi
        - "git fetch origin"
        - "git checkout ${{branch}}"
        - "npm install"
        - "npm run build"
        - "npm run deploy:staging"
  production:
    description: "Deploy to production"
    flags:
      - name: "version"
        short: "v"
        description: "Release version"
    cmd:
      run:
        - "git tag v${{version}}"
        - "git push --tags"
        - "npm run deploy:production"
```

Usage:

```bash
myalias deploy staging -b feature/new-ui
myalias deploy production -v 2.1.0
```

### Example 2: Multi-Config Infra CLI

`references.yml`:

```yaml
banner: "Infrastructure CLI"
configs:
  - deploy.yml
  - db.yml
  - monitoring.yml
```

`deploy.yml`:

```yaml
version: "2.0"
name: "deploy"
description: "Deployment"
commands:
  status:
    description: "Check deployment status"
    flags: []
    cmd:
      run:
        - "kubectl get deployments -A"
  restart:
    description: "Restart a service"
    flags:
      - name: "service"
        short: "s"
        description: "Service name"
    cmd:
      run:
        - "kubectl rollout restart deployment/${{service}} -n production"
```

`db.yml`:

```yaml
version: "2.0"
name: "db"
description: "Database operations"
commands:
  migrate:
    description: "Run migrations"
    flags:
      - name: "target"
        description: "Target migration version"
    cmd:
      run:
        - "npm run migrate:run"
  backup:
    description: "Backup database"
    flags: []
    cmd:
      run:
        - "pg_dump -U postgres mydb > backup-$(date +%s).sql"
```

`monitoring.yml`:

```yaml
version: "2.0"
name: "monitoring"
description: "Monitoring and alerts"
commands:
  logs:
    description: "View service logs"
    flags:
      - name: "service"
        short: "s"
        description: "Service name"
      - name: "lines"
        short: "n"
        description: "Number of lines"
    cmd:
      run:
        - "journalctl -u ${{service}} -n ${{lines}} -f"
  alert:
    description: "Manage alerts"
    subcommands:
      list:
        description: "List active alerts"
        flags: []
        cmd:
          run:
            - "curl http://alertmanager:9093/api/v1/alerts"
      silence:
        description: "Silence an alert"
        flags:
          - name: "id"
            description: "Alert ID"
          - name: "duration"
            description: "Silence duration (e.g., 30m)"
        cmd:
          run:
            - |
              curl -X POST http://alertmanager:9093/api/v1/alerts/groups \
                -H 'Content-Type: application/json' \
                -d '{
                  "groupLabels": {"alertname": "${{id}}"},
                  "matchers": [{"isEqual": true, "isRegex": false, "name": "alertname", "value": "${{id}}"}],
                  "startsAt": "'"$(date -u +%Y-%m-%dT%H:%M:%SZ)"'",
                  "duration": "${{duration}}"
                }'
```

Usage:

```bash
infra deploy status
infra db migrate
infra monitoring logs -s nginx -n 50
infra monitoring alert list
infra monitoring alert silence --id DiskFull --duration 1h
```

### Example 3: Nested Subcommands

`cloud.yml`:

```yaml
version: "2.0"
name: "cloud"
description: "Cloud infrastructure"
commands:
  aws:
    description: "AWS operations"
    subcommands:
      ec2:
        description: "EC2 instances"
        subcommands:
          list:
            description: "List instances"
            flags: []
            cmd:
              run:
                - "aws ec2 describe-instances --region us-east-1"
          terminate:
            description: "Terminate an instance"
            flags:
              - name: "instance-id"
                description: "EC2 instance ID"
            cmd:
              run:
                - "aws ec2 terminate-instances --instance-ids ${{instance-id}} --region us-east-1"
      rds:
        description: "RDS databases"
        subcommands:
          snapshot:
            description: "Create a snapshot"
            flags:
              - name: "db-id"
                description: "Database identifier"
            cmd:
              run:
                - "aws rds create-db-snapshot --db-instance-identifier ${{db-id}}"
  gcp:
    description: "Google Cloud"
    subcommands:
      compute:
        description: "Compute Engine"
        subcommands:
          list:
            description: "List instances"
            flags: []
            cmd:
              run:
                - "gcloud compute instances list"
```

Usage:

```bash
myalias cloud aws ec2 list
myalias cloud aws ec2 terminate --instance-id i-1234567890abcdef0
myalias cloud aws rds snapshot --db-id production-db
myalias cloud gcp compute list
```

## Tips and Best Practices

### Use Descriptive Flag Names

Good:

```yaml
- name: "output-format"
  description: "Output format (json, yaml, table)"
```

Bad:

```yaml
- name: "fmt"
  description: "Format"
```

### Organize with Subcommands

Instead of flat commands, use nesting:

Bad:

```yaml
commands:
  deploy_staging:
    ...
  deploy_production:
    ...
  backup_staging:
    ...
  backup_production:
    ...
```

Good:

```yaml
commands:
  deploy:
    subcommands:
      staging: ...
      production: ...
  backup:
    subcommands:
      staging: ...
      production: ...
```

### Document with Banners

Use banners to warn about dangerous operations:

```yaml
banner: "WARNING: This is the production database. Use with caution."
```

### Set base-dir for Project Commands

If commands assume a specific working directory, set `base-dir`:

```yaml
base-dir: "/home/app/myproject"
commands:
  build:
    cmd:
      run:
        - "make build"  # Runs in /home/app/myproject
```

### Use Environment Variables for Secrets

Never hardcode secrets:

Bad:

```yaml
cmd:
  run:
    - "curl -H 'Authorization: Bearer my-secret-token' ..."
```

Good:

```yaml
cmd:
  run:
    - "curl -H 'Authorization: Bearer ${{env.API_TOKEN}}' ..."
```

Then set `export API_TOKEN="my-secret-token"` before using the command.

## Schema Validation

Ring-cli validates configs at init time and reports errors clearly. Common validation errors:

### Missing Required Fields

```
Error: config 'deploy': 'commands.staging' is missing required field 'description'
```

Every command must have a `description`.

### Both cmd and subcommands

```
Error: config 'deploy': 'commands.staging' has both 'cmd' and 'subcommands'. Choose one.
```

A command must have either `cmd` or `subcommands`, not both.

### Neither cmd nor subcommands

```
Error: config 'deploy': 'commands.staging' has neither 'cmd' nor 'subcommands'. Choose one.
```

A command must have at least one of them.

### Invalid version

```
Error: config 'deploy': invalid version '1.0'. Must be '2.0'.
```

Configs must use version 2.0.
