# OpenAPI Support, README Restructure & Source Reorganization

**Date:** 2026-03-18
**Status:** Approved

## Overview

Add OpenAPI 3.0+ support to ring-cli, restructure the README for marketing, clean up dependencies, and reorganize the source code for maintainability.

Ring-cli transforms OpenAPI specs into standard `Configuration` structs at init time. Once transformed, everything else (caching, refresh, CLI building, execution) works unchanged. The binary maintains zero network capabilities — remote specs are fetched via the user's own curl/wget with explicit consent.

## Key Decisions

1. **Path-based command hierarchy** from OpenAPI paths (e.g., `/pets/{id}` -> `pets get --pet-id 5`)
2. **curl/wget for generated commands** — auto-detected at init, no native HTTP in the binary
3. **Transform at init time**, re-transform on refresh
4. **Auth via environment variables** derived from security scheme names
5. **Dot-notation flags** for nested request bodies, unlimited depth
6. **Remote spec fetching** via curl/wget with consent prompt, `--yes` to skip
7. **OpenAPI 3.0+ now**, extensible for Swagger 2.0 later
8. **Skip unsupported features** with warnings, best-effort approximations where possible
9. **Remove reqwest/tokio**, add openapiv3
10. **README becomes marketing + features**, current content moves to docs
11. **Source restructured** — extract main.rs into focused modules, openapi/ submodule

## 1. OpenAPI Parser & Transformer

New module: `src/openapi/`

**Input:** Raw OpenAPI 3.0+ spec (JSON or YAML)
**Output:** `Configuration` struct ready for caching

### Path-to-Command Mapping

```
GET    /pets              ->  pets list
POST   /pets              ->  pets create
GET    /pets/{petId}      ->  pets get --pet-id <value>
DELETE /pets/{petId}      ->  pets delete --pet-id <value>
GET    /pets/{petId}/toys ->  pets toys list --pet-id <value>
POST   /pets/{petId}/toys ->  pets toys create --pet-id <value>
```

Rules:
- Path segments become subcommand hierarchy
- Path parameters become flags at the deepest command level, inherited by children
- HTTP method maps to verb: GET (collection) -> `list`, GET (item) -> `get`, POST -> `create`, PUT -> `update`, PATCH -> `patch`, DELETE -> `delete`
- Collection vs item: the *last* segment determines this. If the last segment is a `{param}`, it is an item operation; otherwise it is a collection operation
- `operationId` used for description if available, otherwise auto-generated
- `Configuration.name` for OpenAPI specs is derived from `info.title` (slugified to lowercase, hyphens for spaces). If missing, derived from the filename

### Flag Generation from Request Bodies

Dot-notation flattening, unlimited depth:

```yaml
# OpenAPI schema:
properties:
  name: { type: string, description: "Pet name" }
  owner:
    type: object
    properties:
      email: { type: string }
      address:
        type: object
        properties:
          city: { type: string }
```

Generates flags:
```
--name              "Pet name"
--owner.email       (string)
--owner.address.city (string)
```

### Dot-Notation Flags and Clap Compatibility

Clap does not natively support dots in flag names. The implementation must:
- Use the dot-notation string as the clap `Arg::id()` (internal identifier)
- Use the dot-notation string as `.long()` (the `--flag-name` users type)
- Verify clap accepts dots in `.long()` during implementation; if not, use hyphens on the CLI (`--owner-email`) and map back to dot-notation internally for JSON body assembly
- The JSON body builder reconstructs nested structure from the flat flag map regardless of CLI representation

Each generated curl/wget command is a single string in `CmdType.run` (one entry per `run` vector element). Multi-line commands use escaped newlines or are joined into a single line.

Rules:
- Flag descriptions pulled from schema `description` fields
- Required fields noted in description
- Array types: flag can be passed multiple times

### Query Parameters & Headers

- Query params become flags: `?limit=10` -> `--limit`
- Custom headers from spec become flags: `X-Request-Id` -> `--x-request-id`
- Standard headers (Content-Type, Accept) auto-set from operation

### curl/wget Command Generation

curl example for POST /pets:
```sh
curl -s -X POST https://api.example.com/pets \
  -H 'Content-Type: application/json' \
  -H 'Authorization: Bearer ${{env.API_TOKEN}}' \
  -d '{"name":"${{name}}","owner":{"email":"${{owner.email}}","address":{"city":"${{owner.address.city}}"}}}'
```

wget equivalent:
```sh
wget -q -O- --method=POST https://api.example.com/pets \
  --header='Content-Type: application/json' \
  --header='Authorization: Bearer ${{env.API_TOKEN}}' \
  --body-data='{"name":"${{name}}","owner":{"email":"${{owner.email}}","address":{"city":"${{owner.address.city}}"}}}'
```

### Authentication

- Derive env var names from security scheme names in the spec
- e.g., scheme `bearerAuth` -> `${{env.BEARER_AUTH_TOKEN}}`
- Injected into generated curl/wget commands as headers

### Unsupported Features

| Feature | Behavior |
|---------|----------|
| `oneOf`/`anyOf` schemas | Best-effort: generate flags for all possible fields, warn |
| File uploads (multipart/form-data) | Best-effort: generate `--file` flag with curl `-F` |
| Websocket endpoints | Skip with warning |
| XML-only content types | Skip with warning |
| Callbacks/webhooks | Skip with warning |
| `allOf` schemas | Merge all schemas, generate combined flags |

Init stderr output summary:
```
Generated 12 commands from OpenAPI spec
Skipped 2 operations (websocket, xml-only)
3 operations use best-effort approximations (use --verbose for details)
```

### OpenAPI Version Handling

- Support OpenAPI 3.0+ via `openapiv3` crate
- Reject Swagger 2.0 with clear error: "Swagger 2.0 not yet supported, use OpenAPI 3.0+"
- Design transformation behind a trait for future extensibility

## 2. HTTP Tool Detection & Remote Fetching

### Tool Detection

Priority order at init time:
1. `curl --version` -> use curl
2. `wget --version` -> use wget
3. Neither -> error: "curl or wget is required for OpenAPI support"

The detected tool is stored in `AliasMetadata` as a hint for refresh. At refresh time, the stored tool is validated (check it still exists); if not, re-detect from scratch. This way metadata avoids a redundant detection step in the common case, but handles tool changes gracefully.

### Remote Fetch Flow

For `--config-path openapi:https://...`:

```
1. Detect HTTP tool
2. Prompt on stderr (unless --yes):
   "Warning: ring-cli will use 'curl' to download https://..."
   "Continue? [Y/n]"
3. Fetch: curl -s -f -L <url>  or  wget -q -O- <url>
4. Validate response is valid OpenAPI spec
5. Proceed with transformation
```

### Local File Flow

For `--config-path openapi:./spec.yml`:
- Read file directly, no prompt
- Still detect HTTP tool for command generation

### Output Conventions (entire feature)

- **stdout** — only command output
- **stderr** — all ring-cli messages: warnings, prompts, progress, errors
- **No emojis** — ASCII only, POSIX-safe
- **Colors** — existing style.rs system (NO_COLOR, --color, pipe detection)
- **Exit codes** — 0 success, 1 for all errors (matches clap default behavior)

## 3. Config Path Prefix Routing

### Prefix Detection

```
--config-path openapi:https://api.example.com/spec.yml  -> remote OpenAPI
--config-path openapi:./local/spec.yml                   -> local OpenAPI
--config-path openapi:/absolute/path/spec.yml            -> local OpenAPI
--config-path ./regular-config.yml                       -> existing behavior
```

Detection logic: check for literal `openapi:` prefix. After stripping the prefix, classify as remote if the remainder starts with `http://` or `https://`, otherwise treat as a local file path. This avoids ambiguity with Windows drive letters (e.g., `C:\...` would never start with `openapi:`).

Multiple `--config-path` flags can mix regular and OpenAPI configs:
```
ring-cli init --alias mytools \
  --config-path ./deploy.yml \
  --config-path openapi:./api-spec.yml
```

### References File Support

```yaml
banner: "My tools"
configs:
  - ./deploy.yml
  - openapi:./api-spec.yml
  - openapi:https://api.example.com/openapi.yml
```

### Metadata Changes

```rust
struct ConfigEntry {
    name: String,
    source_path: String,       // retains "openapi:" prefix for refresh
    hash: String,              // hash of the RAW source (OpenAPI spec or YAML config)
    trusted_at: u64,
}

struct AliasMetadata {
    configs: Vec<ConfigEntry>,
    banner: Option<String>,
    http_tool: Option<String>,  // "curl" or "wget", hint for refresh (re-validated at use)
}
```

### Caching and Hashing Strategy

For OpenAPI configs, two things are stored:
- **Cached file:** the *transformed* Configuration YAML (what cli.rs consumes)
- **Hash:** computed from the *raw OpenAPI spec* (the source of truth)

This means: if the OpenAPI spec changes at all, the hash changes and refresh detects it, even if the transformation output happens to be identical. This matches regular configs where the hash is always of the source content.

The `resolve_references()` function must be updated to handle `openapi:` prefixed entries — skip filesystem path joining and existence checks for these, and route them through the OpenAPI fetch/transform pipeline instead.

### Init Flow

```
1. Parse --config-path values
2. For each path:
   a. If starts with "openapi:":
      - Detect HTTP tool (once, cached for session)
      - If remote URL: prompt for consent (unless --yes), fetch
      - If local: read file
      - Parse OpenAPI spec
      - Transform to Configuration
   b. Else:
      - Existing behavior
3. Check for name conflicts across all configs
4. Cache via save_trusted_configs()
5. Install shell function, completions, etc.
```

## 4. Refresh Integration

### Refresh Flow for OpenAPI Configs

```
1. Load metadata from cache
2. For each ConfigEntry:
   a. If source_path starts with "openapi:":
      - Extract path/URL after prefix
      - If remote: prompt for consent (unless --yes), fetch via stored http_tool
      - If local: read file
      - Parse and transform OpenAPI spec
      - Compare hash of raw spec with cached hash
      - If changed: prompt to trust (existing flow)
   b. Else:
      - Existing refresh behavior
3. Save updated configs
```

### Edge Cases

- **Remote URL down:** warn on stderr, keep cached copy, continue
- **HTTP tool no longer installed:** error with message identifying the missing tool
- **Local spec deleted:** warn on stderr, keep cached copy (matches existing behavior)

## 5. Dependency Changes

Note: reqwest and tokio were already removed in a prior modernization. Verify they are absent.

### Add

- **openapiv3** — OpenAPI 3.0 spec parser, pure Rust, no network capabilities

### Verify

- Confirm reqwest/tokio are not in Cargo.toml or Cargo.lock
- `cargo build` succeeds with openapiv3 added
- All tests pass
- `cargo audit` clean

## 5a. New CLI Flags

### `--yes` flag

Add `--yes` to both `ring-cli init` and `refresh-configuration`:
- On `init`: skips the download consent prompt for remote OpenAPI specs
- On `refresh-configuration`: skips both download consent and trust confirmation prompts
- Useful for CI/CD, automation, and AI agent workflows

### `--verbose` on init

Add `--verbose` to `ring-cli init` to show detailed warnings about best-effort approximations and skipped operations during OpenAPI transformation.

## 6. Source Code Restructuring

### Current State

`main.rs` at ~740 lines handles init, refresh, shell detection, alias installation, completions, update checking, references, and dispatch. Too many concerns.

### Target Structure

```
src/
  main.rs              -- Entry point: arg parsing, mode dispatch (~100 lines)
  init.rs              -- Init flow, default config creation, references
  refresh.rs           -- Refresh and update-check logic
  shell.rs             -- Shell detection, alias install, completion hooks
  cli.rs               -- Clap builder, command execution
  config.rs            -- Config loading, validation, placeholders (replaces utils.rs)
  models.rs            -- Data structures
  cache.rs             -- Trust storage, metadata
  style.rs             -- Color output
  errors.rs            -- Error types
  openapi/
    mod.rs             -- Public API: transform_spec() -> Configuration
    parser.rs          -- OpenAPI spec parsing via openapiv3
    transform.rs       -- Spec-to-Configuration, path hierarchy, flag generation
    http_tool.rs       -- curl/wget detection, command generation, remote fetching
```

### Extraction from main.rs

| Functions | New home |
|-----------|----------|
| `handle_init()`, `create_default_config()`, `resolve_references()`, `validate_alias_name()` | init.rs |
| `handle_refresh_configuration()`, `handle_check_updates()` | refresh.rs |
| `detect_shell_configs()`, `ShellConfig`, `ShellKind`, `alias_exists()`, `clean_alias_from_shells()`, `append_to_shell_config()`, completion hooks | shell.rs |
| `load_configuration()`, `replace_placeholders()`, `replace_env_vars()`, validation | config.rs (replaces utils.rs) |

main.rs retains only: `fn main()`, top-level arg definitions, mode dispatch.

### Guidelines

- No behavior changes -- pure restructuring pass first, features second
- Each file has one clear responsibility
- openapi/ is a submodule from the start
- All existing tests pass after restructuring

## 7. README Restructure & Documentation

### New README.md

Marketing-focused, aimed at devs, devops, and AI agents:

```
# ring-cli

Tagline

## Why ring-cli
  Value proposition (4-5 sentences)

## Install
  curl oneliner
  brew / cargo / source alternatives

## Quick Start (YAML)
  3-step example

## Quick Start (OpenAPI)
  3-step example

## Features
  - YAML-Driven CLI Generation
  - OpenAPI Support
  - Tab Completion at Every Level (bash, zsh, fish, powershell)
  - Multi-Config Composition
  - Trust-Based Security Model
  - Zero Network Footprint
  - Cross-Platform (20+ targets)
  - Built for Automation (stdout/stderr, quiet mode, --yes, ASCII-only)
  - Environment & Flag Variables
  - Verbose Mode for Debugging
  - Nested Subcommands (unlimited depth)
  - Configurable Banners
  - NO_COLOR Standard Support

## Documentation
  Links to guides

## License
```

### Documentation Files

```
docs/
  getting-started.md           -- Current README content, expanded
  openapi-guide.md             -- OpenAPI usage, examples, limitations
  configuration-reference.md   -- YAML schema, validation, examples
  setup-guide.md               -- Existing, updated
```

### install.sh

- Detect OS and architecture
- Download correct binary from GitHub releases
- Install to ~/.local/bin or /usr/local/bin
- curl with wget fallback for self-download
- Clear error if platform not supported

### Tone

- Technical but approachable
- Show, don't tell -- real commands throughout
- No emojis, no buzzwords
- Security story as a feature, not a disclaimer

## 8. Test Strategy

### Real-World OpenAPI Fixtures

- Petstore (classic simple API)
- Petstore Expanded (nested schemas, auth)
- Deep nesting spec (3+ levels for dot-notation)
- Mixed content types (skip/best-effort paths)
- Multiple security schemes
- Missing servers field (edge case)

### Unit Tests (openapi/)

- Path-to-hierarchy mapping (flat, nested, parameterized)
- Method-to-verb mapping (collection vs item detection)
- Dot-notation flattening at various depths
- curl command generation
- wget command generation
- Flag generation from query params, headers, path params, body
- JSON body assembly from flattened flags
- Security scheme -> env var name derivation
- Unsupported feature detection and warnings

### Integration Tests

- Full init-to-execution flow with OpenAPI spec
- Mixed regular + OpenAPI configs
- Remote spec fetching (mocked or test server)
- Refresh with changed spec
- --yes flag behavior
- References file with openapi: entries
- Name conflicts between regular and OpenAPI configs
- All output on correct stream (stdout vs stderr)
- ASCII-only output verification

### Existing Tests

- All current tests pass after restructuring (no behavior changes)
- install.sh platform detection and download

## Files Changed

### New Files

- `src/init.rs`
- `src/refresh.rs`
- `src/shell.rs`
- `src/config.rs` (replaces utils.rs)
- `src/openapi/mod.rs`
- `src/openapi/parser.rs`
- `src/openapi/transform.rs`
- `src/openapi/http_tool.rs`
- `docs/getting-started.md`
- `docs/openapi-guide.md`
- `docs/configuration-reference.md`
- `install.sh`
- `tests/fixtures/petstore.yml` + other OpenAPI fixtures
- `README.md` (full rewrite)

### Modified Files

- `Cargo.toml` — add openapiv3, verify no reqwest/tokio
- `src/main.rs` — reduced to entry point + dispatch
- `src/cache.rs` — http_tool field in AliasMetadata
- `tests/integration.rs` — new OpenAPI tests
- `AGENTS.md` — updated project structure
- `docs/setup-guide.md` — updated

### Removed Files

- `src/utils.rs` (replaced by config.rs)

### Potentially Modified

- `src/models.rs` — may need adjustments if clap does not support dot-notation in `.long()` flag names; in that case, Flag struct gets an optional `internal_id` field for JSON body mapping

### Unchanged

- `src/style.rs` — existing color system reused
- `src/errors.rs` — existing error types reused
- All existing YAML config behavior — fully backward compatible
