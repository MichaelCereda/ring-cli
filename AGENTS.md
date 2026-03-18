# Ring-CLI Agent Instructions

Ring-CLI generates CLIs from YAML configs and OpenAPI specs. See [README.md](README.md) for features and [docs/](docs/) for full documentation.

## Build & Test

Requires Rust toolchain via mise:

```bash
eval "$(mise activate zsh)" && cargo build
eval "$(mise activate zsh)" && cargo test
```

## Project Structure

```
src/
  main.rs          -- Entry point, mode dispatch (~230 lines)
  init.rs          -- Init flow, default config creation, references, base-dir resolution
  refresh.rs       -- Refresh and update-check logic
  shell.rs         -- Shell detection, alias install, completion hooks
  cli.rs           -- CLI construction (clap builder API), command execution
  config.rs        -- Config loading, validation, placeholder/env-var replacement
  models.rs        -- YAML data structures (Configuration, Command, Flag, CmdType)
  cache.rs         -- Trusted config storage (~/.ring-cli/aliases/), SHA-256 hashing
  style.rs         -- Color output (ANSI, NO_COLOR, --color flag)
  errors.rs        -- Error types
  openapi/
    mod.rs         -- Public API: process_openapi_source()
    parser.rs      -- OpenAPI 3.0 spec parsing via openapiv3
    transform.rs   -- Spec-to-Configuration transformation, path hierarchy, flag generation
    http_tool.rs   -- curl/wget detection, command generation, remote fetching
tests/
  integration.rs   -- End-to-end CLI tests (init, completions, OpenAPI, live shell tests)
  fixtures/        -- Test YAML configs and OpenAPI specs
docs/
  getting-started.md          -- Step-by-step setup guide
  configuration-reference.md  -- Full YAML schema reference
  openapi-guide.md            -- OpenAPI usage guide
  setup-guide.md              -- Installation and setup
```

## Architecture

- **Dynamic CLI from YAML** -- commands are loaded at runtime, must use clap builder API (not derive)
- **Two modes:** installer (`ring-cli init`) and alias (`ring-cli --alias-mode <name>`)
- **OpenAPI support:** specs transformed to Configuration structs at init time, commands use curl/wget
- **Trust model:** configs cached with SHA-256 in `~/.ring-cli/aliases/<name>/`
- **Zero network footprint:** no HTTP client in the binary, curl/wget for external tools only
- **Placeholder syntax:** `${{flag_name}}` and `${{env.VAR_NAME}}`
- **Shell support:** bash, zsh, fish, powershell (functions + tab completion)
- **Output:** stdout for command output only, stderr for all ring-cli messages
- **POSIX-safe:** ASCII-only output, no emojis

## Code Conventions

- Each command must have exactly one of `cmd` or `subcommands`, not both
- Error handling: `RingError` enum with thiserror, `anyhow` at top level
- YAML parsing: `serde-saphyr` (panic-free, maintained)
- OpenAPI parsing: `openapiv3` (pure Rust, no network)
- Colors respect `NO_COLOR` env var and `--color` flag
- Tests that depend on Unix shell behavior must be skipped on Windows with `#[cfg_attr(target_os = "windows", ignore = "reason")]`

## Git Conventions

- Do not add `Co-Authored-By` trailers to commit messages
- Commit messages should be concise and describe the "why"

## Key Dependencies

- clap 4.5 (with "string" feature for dynamic CLI building)
- openapiv3 2 (OpenAPI 3.0 parsing)
- serde-saphyr 0.0.21 (YAML)
- dirs 6.0, thiserror 2.0, anyhow 1.0
