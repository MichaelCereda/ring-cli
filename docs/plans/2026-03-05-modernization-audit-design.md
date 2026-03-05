# Ring-CLI Modernization & Audit Design

## Context

Ring-CLI is a YAML-driven CLI tool (~377 lines of Rust) for defining custom commands, subcommands, and HTTP requests. It targets 20+ platforms via cross-compilation.

An audit revealed 12 security vulnerabilities, 10 dependency warnings, zero tests, and several outdated dependencies (clap v2, serde_yaml v0.8, reqwest v0.11). This design covers a full modernization pass.

**Audience**: Personal/team developer tool (trusted configs).

## 1. Dependency Modernization

### Upgrades

| Dependency | Current | Target | Notes |
|---|---|---|---|
| clap | 2.33 | 4.x | Builder API (commands are dynamic from YAML) |
| serde | 1.0 | 1.x latest | Minor bump |
| serde_derive | 1.0 | Remove | Use serde's `derive` feature |
| serde_yaml | 0.8 | Replace with `serde_yml` | serde_yaml and yaml-rust are unmaintained |
| reqwest | 0.11.18 | 0.12+ | Fixes h2, openssl, webpki, ring CVEs |
| tokio | 1 (full) | 1.x latest | Trim features to `rt-multi-thread, macros` |
| dirs | 3.0 | 5.x | Latest |
| openssl | 0.10.56 | Remove | Use rustls-tls everywhere |

### New Dependencies

| Dependency | Purpose |
|---|---|
| thiserror | Proper error types |
| anyhow | Main-level error handling |

### TLS Strategy

Drop vendored OpenSSL entirely. Use `rustls-tls` for all platforms. This:
- Eliminates 3 OpenSSL CVEs
- Removes complex platform-conditional Cargo.toml blocks
- Simplifies cross-compilation (no vendored C code)
- rustls is pure Rust, audited, well-maintained

## 2. CLI Rewrite (cli.rs)

Migrate from clap v2 to v4 builder API. Key API changes:

- `App::new()` -> `Command::new()`
- `SubCommand::with_name()` -> `Command::new()`
- `Arg::with_name().short("x")` -> `Arg::new().short('x')`
- `matches.is_present()` -> `matches.get_flag()`
- `matches.value_of()` -> `matches.get_one::<String>()`
- `matches.args.iter()` (internal) -> iterate over known flag names from Command model

The biggest change: clap v2 code directly accesses `matches.args` in `replace_placeholders` and `execute_http_request`. clap v4 doesn't expose internals. Iterate over known flag names from the YAML Command model instead.

Keep `tokio::runtime::Runtime::new().block_on()` for HTTP — main is sync, HTTP is occasional.

Replace `Result<_, String>` with `Result<_, RingError>` using thiserror.

## 3. New Features

### 3a. Environment Variable Support

Extend placeholders to support `${{env.VAR_NAME}}`:

```yaml
cmd:
  run:
    - "curl -H 'Authorization: Bearer ${{env.API_TOKEN}}' https://${{target}}/deploy"
```

After replacing flag values, scan for `${{env.XXX}}` and replace with `std::env::var("XXX")`. Error if env var is not set.

### 3b. Better Error Messages

- Validation errors include command path: `"mycli > deploy > staging: Either 'cmd' or 'subcommands' must be present"`
- YAML parse errors show file path: `"Error parsing '/path/to/file.yml': expected string at line 5, column 3"`
- HTTP errors include method + URL: `"POST https://api.example.com: connection refused"`
- Shell failures include command and exit code

### 3c. Config Init / Scaffolding

Built-in `init` subcommand:

```
ring-cli init [--path ./my-config.yml]
```

Generates starter YAML with commented examples. Defaults to `~/.ring-cli/configurations/example.yml`. Handled before config loading so it works with no existing configs.

## 4. Test Suite

### Unit Tests

**models.rs**: Config deserialization, validation (cmd+subcommands rejected, neither rejected, recursive), flag parsing, CmdType variants.

**utils.rs**: Placeholder replacement (single/multiple flags, missing flags, env vars), config loading (single file, directory, nonexistent path, invalid YAML).

**cli.rs**: CLI structure from configs, nested subcommands, global flags present.

### Integration Tests

**tests/integration.rs**: End-to-end config load + command execution, --help output verification, init command.

**tests/fixtures/**: Sample YAML configs for testing.

## 5. CI/CD Updates

- Bump actions: checkout v3 -> v4, upload-artifact v3 -> v4, action-gh-release v1 -> v2
- ubuntu-20.04 -> ubuntu-latest
- Fix Changes.md reference in release step
- Keep all 20 build targets
- Simpler cross-compilation with rustls-tls (no vendored OpenSSL)

## 6. File Changes

**Modified**: `Cargo.toml`, `src/main.rs`, `src/cli.rs`, `src/models.rs`, `src/utils.rs`, `.github/workflows/ci.yml`

**Added**: `src/errors.rs`, `tests/integration.rs`, `tests/fixtures/*.yml`, `mise.toml`

## 7. Out of Scope

- Shell command injection hardening (trusted user context)
- Async rewrite of main
- Plugin system
- Config file watching
