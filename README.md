# Ring-CLI

Ring-CLI generates custom command-line tools from YAML configuration files. Define your commands, flags, and subcommands in YAML, then install them as a shell alias with automatic tab completion, a trust-based security model, and color output.

## Quick Start

```bash
# Install ring-cli, then create your first alias
ring-cli init --alias infra --config-path deploy.yml --config-path db.yml
```

This reads your YAML configs, caches them securely, installs a shell alias, and sets up tab completion. Now you can run:

```
infra deploy staging --branch main
infra db migrate
```

## Configuration Format

Each configuration file defines a named group of commands:

```yaml
version: "2.0"
name: "deploy"
description: "Deployment operations"
base-dir: "/opt/infrastructure"  # optional working directory
commands:
  staging:
    description: "Deploy to staging"
    flags:
      - name: "branch"
        short: "b"
        description: "Branch to deploy"
    cmd:
      run:
        - "echo Deploying ${{branch}} to staging"
  prod:
    description: "Deploy to production"
    flags:
      - name: "tag"
        short: "t"
        description: "Release tag"
    cmd:
      run:
        - "echo Deploying ${{tag}} to prod"
```

```yaml
version: "2.0"
name: "db"
description: "Database operations"
commands:
  migrate:
    description: "Run database migrations"
    flags: []
    cmd:
      run:
        - "echo Running migrations..."
  seed:
    description: "Seed database"
    flags: []
    cmd:
      run:
        - "echo Seeding database..."
```

### Config Fields

| Field         | Required | Description                                              |
|---------------|----------|----------------------------------------------------------|
| `version`     | Yes      | Config format version. Must be `"2.0"`.                  |
| `name`        | Yes      | Name for this config group. Becomes a top-level command.  |
| `description` | Yes      | Description shown in `--help` output.                    |
| `base-dir`    | No       | Working directory for all commands in this config.        |
| `commands`    | Yes      | Map of command names to command definitions.              |

### Command Fields

| Field         | Required | Description                                              |
|---------------|----------|----------------------------------------------------------|
| `description` | Yes      | Description shown in `--help` output.                    |
| `flags`       | No       | List of flags the command accepts.                       |
| `cmd`         | *        | Command to execute (`run` or `http`). Required if no `subcommands`. |
| `subcommands` | *       | Nested subcommands. Required if no `cmd`.                |

A command must have either `cmd` or `subcommands`, not both.

### Flag Fields

| Field         | Required | Description                                              |
|---------------|----------|----------------------------------------------------------|
| `name`        | Yes      | Flag name (used as `--name`).                            |
| `short`       | No       | Single-character short form (used as `-n`).              |
| `description` | Yes      | Description shown in `--help` output.                    |

## Multiple Configs Per Alias

An alias can combine multiple configuration files. Each config's `name` becomes a top-level subcommand:

```bash
ring-cli init --alias infra --config-path deploy.yml --config-path db.yml
```

```
infra deploy staging    # from deploy.yml (name: "deploy")
infra db migrate        # from db.yml (name: "db")
```

If two configs use the same `name`, init will error. Use `--warn-only-on-conflict` to downgrade to a warning.

## Shell Commands

Use `${{flag_name}}` to reference flag values and `${{env.VAR_NAME}}` for environment variables:

```yaml
commands:
  deploy:
    description: "Deploy with auth"
    flags:
      - name: "target"
        short: "t"
        description: "Deploy target"
    cmd:
      run:
        - "curl -H 'Authorization: Bearer ${{env.API_TOKEN}}' https://${{target}}/deploy"
```

Multi-step commands run sequentially. If any step fails, execution stops:

```yaml
commands:
  setup:
    description: "Full setup"
    flags: []
    cmd:
      run:
        - "echo Step 1: Installing..."
        - "echo Step 2: Configuring..."
        - "echo Step 3: Done!"
```

## HTTP Commands

Execute HTTP requests directly:

```yaml
commands:
  status:
    description: "Check API status"
    flags: []
    cmd:
      http:
        method: "GET"
        url: "https://api.example.com/status"
  create:
    description: "Create a resource"
    flags:
      - name: "data"
        short: "d"
        description: "JSON body"
    cmd:
      http:
        method: "POST"
        url: "https://api.example.com/resources"
        headers:
          Content-Type: "application/json"
          Authorization: "Bearer ${{env.API_TOKEN}}"
        body: "${{data}}"
```

Supported methods: `GET`, `POST`, `PUT`, `DELETE`, `PATCH`, `HEAD`.

## Nested Subcommands

Commands can be nested arbitrarily deep using `subcommands`:

```yaml
commands:
  cloud:
    description: "Cloud operations"
    flags: []
    subcommands:
      aws:
        description: "AWS operations"
        flags: []
        subcommands:
          deploy:
            description: "Deploy to AWS"
            flags: []
            cmd:
              run:
                - "echo Deploying to AWS..."
```

Usage: `myalias config-name cloud aws deploy`

## Security: Trust System

ring-cli never runs commands from a config file without explicit trust.

1. **`ring-cli init`** reads the YAML, validates it, and stores a trusted copy with a SHA-256 hash in `~/.ring-cli/aliases/<name>/`. The config is auto-trusted since you just pointed to it.

2. **Your alias** runs from the cached/trusted config, not the original YAML file.

3. **`<alias> refresh-configuration`** re-reads the original YAML files, compares hashes, and shows what changed. You must type `y` to trust the new version. If you decline, the old trusted version is kept.

4. **If the original YAML is deleted**, the alias still works from cache. `refresh-configuration` reports the missing source.

### Cache Structure

```
~/.ring-cli/
  aliases/
    <alias-name>/
      <config-name>.yml   # trusted copy of each config
      metadata.json        # source paths, SHA-256 hashes, trust timestamps
```

## Color Output

- **Auto-detected**: Color is enabled when output goes to a terminal, disabled when piped.
- **`NO_COLOR` env var**: Set `NO_COLOR=1` to disable all color ([no-color.org](https://no-color.org) standard).
- **`--color` flag**: Override with `--color=always`, `--color=never`, or `--color=auto` (default).

Only ring-cli's own messages (errors, warnings, success) are colored. Command output is always passed through unmodified.

## Tab Completion

Tab completion is installed automatically during `ring-cli init` for all detected shells:

- **Bash** and **Zsh**
- **Fish**
- **PowerShell**

Completions are generated from the cached config, so they stay fast and consistent. After running `refresh-configuration`, restart your shell to pick up completion changes.

## CLI Reference

### `ring-cli init`

```
ring-cli init --alias <NAME> [--config-path <PATH>]... [--warn-only-on-conflict]
```

| Flag                      | Description                                          |
|---------------------------|------------------------------------------------------|
| `--alias <NAME>`          | Shell alias name to install (required).              |
| `--config-path <PATH>`   | Path to a config file. Repeatable for multiple configs. |
| `--warn-only-on-conflict` | Warn instead of error on config name conflicts.      |

### Alias Commands

Once installed, your alias supports:

```
<alias> [OPTIONS] <config-name> <command> [FLAGS]
<alias> refresh-configuration
```

| Option           | Short | Description                    |
|------------------|-------|--------------------------------|
| `--quiet`        | `-q`  | Suppress error messages.       |
| `--verbose`      | `-v`  | Print verbose output.          |
| `--color <WHEN>` |       | Color output (`auto`, `always`, `never`). |
| `--help`         | `-h`  | Print help.                    |
| `--version`      | `-V`  | Print version.                 |

## Installation

### From Source

```bash
cargo install --path .
```
