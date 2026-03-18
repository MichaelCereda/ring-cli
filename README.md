# ring-cli

Turn YAML configs and OpenAPI specs into fully-featured CLIs with tab completion, security you control, and zero attack surface.

## Why ring-cli

ring-cli is a CLI generator for teams and operators who need custom command-line tools without writing Go, Python, or Rust. Define your commands in YAML, import an OpenAPI spec, or mix both. Get a shell alias with automatic tab completion, nested subcommands, environment variable substitution, and a trust-based security model that puts you in control. The tool runs only on your machine—no network footprint, no external dependencies, no surprises.

## Install

```bash
curl -fsSL https://raw.githubusercontent.com/MichaelCereda/ring-cli/master/install.sh | sh
```

Or via Homebrew:
```bash
brew install michaelcereda/ring-cli/ring-cli
```

Or build from source:
```bash
cargo install ring-cli
```

## Quick Start: From YAML

Define commands in a YAML file:

```yaml
version: "2.0"
name: "deploy"
description: "Deployment operations"
commands:
  staging:
    description: "Deploy to staging"
    flags:
      - name: "branch"
        description: "Branch to deploy"
    cmd:
      run:
        - "echo Deploying ${{branch}} to staging"
```

Install as a shell alias with one command:

```bash
ring-cli init --alias ops --config-path deploy.yml
```

Use it immediately:

```bash
ops deploy staging --branch main
ops deploy --help           # see available commands
ops --help                  # see all commands
ops <TAB>                   # tab completion works at every level
```

## Quick Start: From OpenAPI

Transform an OpenAPI spec into commands in seconds:

```bash
ring-cli init --alias petstore \
  --config-path openapi:https://petstore3.swagger.io/api/v3/openapi.json
```

Now you have a CLI based on the spec:

```bash
petstore pets list
petstore pets get --pet-id 5
petstore pets create --name "Buddy" --tag "dog"
petstore pets delete --pet-id 3
petstore <TAB>              # see all commands and flags
```

## Features

**YAML-Driven CLI Generation** — Define commands, flags, and subcommands in YAML. No Rust required. Multi-command configs. Supports shell commands, scripts, and environment variable substitution.

**OpenAPI 3.0 Support** — Point ring-cli at an OpenAPI spec (local file or remote URL) and get a CLI automatically. Command hierarchy from paths, flags from parameters and request bodies, curl/wget for execution.

**Tab Completion at Every Level** — Bash, Zsh, Fish, and PowerShell completions installed automatically during setup. Works for commands, subcommands, and flags.

**Multi-Config Composition** — Combine multiple YAML configs or OpenAPI specs into a single alias. Each config becomes a top-level subcommand. Or use a references file to manage them together.

**Trust-Based Security Model** — ring-cli caches configs with SHA-256 hashes. Commands run only from your trusted cache, never directly from a file. Use `refresh-configuration` to see diffs and decide what to trust.

**Zero Network Footprint** — No HTTP client in the binary. OpenAPI specs are fetched by curl/wget with your explicit consent. Remote command execution stays local—no callbacks, no analytics, no phone-home.

**Cross-Platform** — Builds for 20+ targets: Linux (x86_64, ARM, MIPS, PowerPC), macOS (Intel and Apple Silicon), Windows (x86_64 and ARM64), and more.

**Built for Automation** — Stdout and stderr separation for reliable piping. Quiet mode with `-q` to suppress banners. `--yes` flag for CI/CD to skip prompts. ASCII-only output for safe log parsing. Nonzero exit codes on error.

**Environment & Flag Variables** — Use `${{flag_name}}` to reference command flags and `${{env.VAR_NAME}}` for environment variables in your commands. Full POSIX-compliant substitution.

**Verbose Mode for Debugging** — Pass `-v` or `--verbose` to see what ring-cli is doing. Useful for troubleshooting config parsing and command execution.

**Nested Subcommands** — Commands can have subcommands, which can have subcommands. Unlimited depth. Organize complex CLIs naturally.

**Configurable Banners** — Display a message when your alias is invoked. Set per-config or globally via a references file. Banners go to stderr so they don't break pipes.

**NO_COLOR Standard Support** — Respects the NO_COLOR environment variable. Auto-detects terminal output. Override with `--color=always`, `--color=never`, or `--color=auto`.

## Documentation

- [Getting Started Guide](docs/getting-started.md) — Detailed walkthrough, configuration format, shell commands, nested subcommands
- [Configuration Reference](docs/configuration-reference.md) — Complete YAML schema and field descriptions
- [OpenAPI Guide](docs/openapi-guide.md) — Using OpenAPI specs, flag mapping, authentication, limitations
- [Setup Guide](docs/setup-guide.md) — Installation from source, platform-specific notes, troubleshooting

## License

MIT
