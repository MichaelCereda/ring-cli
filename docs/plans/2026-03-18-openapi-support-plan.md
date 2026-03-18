# OpenAPI Support Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add OpenAPI 3.0 support, restructure source code, rewrite README, and add documentation.

**Architecture:** OpenAPI specs are transformed into ring-cli's existing `Configuration` structs at init time via a new `src/openapi/` module. Generated commands use curl/wget (auto-detected). The binary has zero network capabilities. Source is restructured by extracting main.rs into focused modules first, then adding the OpenAPI feature on the clean foundation.

**Tech Stack:** Rust, clap 4.5 (builder API), openapiv3 (OpenAPI parsing), serde-saphyr (YAML), sha2 (hashing)

**Spec:** `docs/plans/2026-03-18-openapi-support-design.md`

**Important notes for implementers:**
- The design spec shows `trusted_at: u64` in the `ConfigEntry` struct, but the actual code uses `String`. Do NOT change the type — keep it as `String`.
- The design spec mentions `append_to_shell_config()` in the extraction table, but this function does not exist in main.rs. Ignore it.
- main.rs is ~915 lines (the spec says ~740 — the spec estimate is stale).

---

## Phase 1: Source Code Restructuring

Pure refactoring. No behavior changes. All 43 existing tests must pass after every task.

### Task 1: Extract shell.rs from main.rs

**Files:**
- Create: `src/shell.rs`
- Modify: `src/main.rs`

- [ ] **Step 1: Create `src/shell.rs` with shell types and detection**

Move these items from `src/main.rs` to `src/shell.rs`:
- `enum ShellKind` (lines 18-22)
- `struct ShellConfig` (and its fields: path, kind, display_name)
- `fn detect_shell_configs() -> Vec<ShellConfig>`
- `fn alias_exists(content: &str, name: &str, kind: ShellKind) -> bool`
- `fn alias_line_bash_zsh(name: &str) -> String`
- `fn alias_line_fish(name: &str) -> String`
- `fn alias_line_powershell(name: &str) -> String`
- `fn clean_alias_lines(content: &str, name: &str, kind: ShellKind) -> String`
- `fn clean_alias_from_shells(name: &str) -> Result<(), anyhow::Error>`
- `fn install_alias(name: &str) -> Result<(), anyhow::Error>`
- `fn install_completions(alias_name: &str) -> Result<(), anyhow::Error>`
- `fn install_update_check(alias_name: &str) -> Result<(), anyhow::Error>`
- `fn remove_update_check(alias_name: &str) -> Result<(), anyhow::Error>`

Make all moved items `pub(crate)`. Add `mod shell;` to `main.rs`. Update all call sites in `main.rs` to use `shell::` prefix.

Move the unit tests for these functions into a `#[cfg(test)] mod tests` block inside `shell.rs`.

- [ ] **Step 2: Verify all tests pass**

Run: `eval "$(mise activate zsh)" && cargo test 2>&1`
Expected: All 43 tests pass, zero failures.

- [ ] **Step 3: Verify no duplicate code remains**

Grep main.rs for any of the moved function names to confirm they are only referenced, not defined.

---

### Task 2: Extract config.rs from utils.rs

**Files:**
- Create: `src/config.rs`
- Remove: `src/utils.rs`
- Modify: `src/main.rs`

- [ ] **Step 1: Rename `src/utils.rs` to `src/config.rs`**

Copy all contents of `src/utils.rs` into `src/config.rs`. In `src/main.rs`, change `mod utils;` to `mod config;`. Update all references from `utils::` to `config::` throughout the codebase (main.rs, cli.rs — anywhere `utils::replace_placeholders`, `utils::replace_env_vars`, or `utils::load_configuration` is called). Delete `src/utils.rs`.

- [ ] **Step 2: Verify all tests pass**

Run: `eval "$(mise activate zsh)" && cargo test 2>&1`
Expected: All 43 tests pass.

---

### Task 3: Extract init.rs from main.rs

**Files:**
- Create: `src/init.rs`
- Modify: `src/main.rs`

- [ ] **Step 1: Create `src/init.rs` with init flow**

Move these items from `src/main.rs` to `src/init.rs`:
- `fn default_config_dir() -> PathBuf`
- `fn validate_alias_name(name: &str) -> Result<(), anyhow::Error>`
- `fn create_default_config(path: &std::path::Path) -> Result<(), anyhow::Error>`
- `struct References` and its serde derive
- `fn resolve_references(ref_path: &std::path::Path) -> Result<(Vec<PathBuf>, Option<String>), anyhow::Error>`
- `fn resolve_base_dir(config: &mut models::Configuration, config_file_path: &str)`
- `fn handle_init(...)` and all its logic

`init.rs` will need these imports:
```rust
use crate::{cache, config, models, shell, style};
use std::fs;
use std::path::PathBuf;
```

Make `handle_init` and `validate_alias_name` `pub(crate)`. Add `mod init;` to `main.rs`. Update `main()` to call `init::handle_init(...)`.

Move the unit tests for `validate_alias_name` into `init.rs`.

- [ ] **Step 2: Verify all tests pass**

Run: `eval "$(mise activate zsh)" && cargo test 2>&1`
Expected: All 43 tests pass.

---

### Task 4: Extract refresh.rs from main.rs

**Files:**
- Create: `src/refresh.rs`
- Modify: `src/main.rs`

- [ ] **Step 1: Create `src/refresh.rs` with refresh flow**

Move these items from `src/main.rs` to `src/refresh.rs`:
- `fn handle_refresh_configuration(alias_name: &str) -> Result<(), anyhow::Error>`
- `fn handle_check_updates(alias_name: &str) -> Result<(), anyhow::Error>`

`refresh.rs` will need:
```rust
use crate::{cache, config, init, models, style};
use std::fs;
```

Note: `resolve_base_dir` is used in refresh flows — it lives in `init.rs`, so import it from there. If it's also needed in `main.rs` alias-mode dispatch, make it `pub(crate)` in `init.rs`.

Make both functions `pub(crate)`. Add `mod refresh;` to `main.rs`.

- [ ] **Step 2: Verify all tests pass**

Run: `eval "$(mise activate zsh)" && cargo test 2>&1`
Expected: All 43 tests pass.

---

### Task 5: Verify main.rs is now minimal

**Files:**
- Verify: `src/main.rs`

- [ ] **Step 1: Confirm main.rs is under ~200 lines**

main.rs should now contain only:
- `mod` declarations (cache, cli, config, errors, init, models, refresh, shell, style)
- `fn main()` with mode dispatch logic
- No struct/enum definitions (except possibly argument-related ones used only in main)

- [ ] **Step 2: Run full test suite**

Run: `eval "$(mise activate zsh)" && cargo test 2>&1`
Expected: All 43 tests pass. This confirms the restructuring is complete and correct.

- [ ] **Step 3: Update AGENTS.md project structure**

Update the project structure section in `AGENTS.md` to reflect the new file layout.

---

## Phase 2: Dependencies & Foundation

### Task 6: Add openapiv3 dependency

**Files:**
- Modify: `Cargo.toml`

- [ ] **Step 1: Verify reqwest/tokio are absent**

Read `Cargo.toml` and confirm no reqwest or tokio dependency exists.

- [ ] **Step 2: Add openapiv3 to Cargo.toml**

Add to `[dependencies]`:
```toml
openapiv3 = "2"
```

- [ ] **Step 3: Verify build and tests**

Run: `eval "$(mise activate zsh)" && cargo build 2>&1 && cargo test 2>&1`
Expected: Build succeeds, all 43 tests pass.

---

### Task 7: Add `--yes` and `--verbose` flags to init

**Files:**
- Modify: `src/cli.rs` (the `build_ring_cli()` function)
- Modify: `src/init.rs` (handle_init signature)
- Modify: `src/main.rs` (pass new flags through)

- [ ] **Step 1: Write failing test for `--yes` flag acceptance**

In `tests/integration.rs`, add:
```rust
#[test]
fn test_init_yes_flag_accepted() {
    let dir = tempfile::TempDir::new().expect("tempdir");
    let target = dir.path().join("yes_test.yml");
    let output = cargo_bin()
        .args(["init", "--config-path", target.to_str().unwrap(), "--alias", "yes-test", "--yes", "--force"])
        .output()
        .expect("failed to run");
    assert!(
        output.status.success(),
        "init --yes should be accepted:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `eval "$(mise activate zsh)" && cargo test test_init_yes_flag_accepted 2>&1`
Expected: FAIL — clap does not recognize `--yes`.

- [ ] **Step 3: Add `--yes` and `--verbose` to `build_ring_cli()` in `src/cli.rs`**

In the `init` subcommand definition inside `build_ring_cli()`, add:
```rust
.arg(
    clap::Arg::new("yes")
        .long("yes")
        .help("Skip confirmation prompts (for CI/automation)")
        .action(clap::ArgAction::SetTrue),
)
.arg(
    clap::Arg::new("verbose")
        .long("verbose")
        .short('v')
        .help("Show detailed output during init")
        .action(clap::ArgAction::SetTrue),
)
```

- [ ] **Step 4: Thread flags through main.rs -> init.rs**

In `main.rs`, extract the new flags from matches and pass them to `init::handle_init()`. Update `handle_init`'s signature to accept `yes: bool` and `verbose: bool`. For now, these flags are accepted but unused (they'll be used when OpenAPI support lands).

- [ ] **Step 5: Run tests**

Run: `eval "$(mise activate zsh)" && cargo test 2>&1`
Expected: All tests pass, including the new `test_init_yes_flag_accepted`.

---

### Task 8: Add `http_tool` field to AliasMetadata

**Files:**
- Modify: `src/cache.rs`

- [ ] **Step 1: Write failing test for http_tool serialization**

In `src/cache.rs` tests, add:
```rust
#[test]
fn test_metadata_http_tool_round_trip() {
    let dir = tempfile::TempDir::new().unwrap();
    let original_home = std::env::var("HOME").ok();
    std::env::set_var("HOME", dir.path());

    let configs = vec![
        ("test".to_string(), "/tmp/test.yml".to_string(), "content".to_string()),
    ];
    save_trusted_configs("httptool-test", &configs, Some("banner".to_string())).unwrap();

    // Manually update metadata to include http_tool
    let meta_path = alias_dir("httptool-test").join("metadata.json");
    let raw = std::fs::read_to_string(&meta_path).unwrap();
    let mut meta: AliasMetadata = serde_json::from_str(&raw).unwrap();
    meta.http_tool = Some("curl".to_string());
    std::fs::write(&meta_path, serde_json::to_string_pretty(&meta).unwrap()).unwrap();

    let (_, loaded_meta) = load_trusted_configs("httptool-test").unwrap();
    assert_eq!(loaded_meta.http_tool, Some("curl".to_string()));

    if let Some(home) = original_home {
        std::env::set_var("HOME", home);
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `eval "$(mise activate zsh)" && cargo test test_metadata_http_tool_round_trip 2>&1`
Expected: FAIL — `AliasMetadata` has no `http_tool` field.

- [ ] **Step 3: Add `http_tool` field to `AliasMetadata`**

In `src/cache.rs`, update:
```rust
#[derive(serde::Serialize, serde::Deserialize, Debug)]
pub struct AliasMetadata {
    pub configs: Vec<ConfigEntry>,
    pub banner: Option<String>,
    #[serde(default)]
    pub http_tool: Option<String>,
}
```

The `#[serde(default)]` ensures backward compatibility — existing metadata.json files without `http_tool` will deserialize with `None`.

- [ ] **Step 4: Update `save_trusted_configs` to accept `http_tool`**

Modify the `save_trusted_configs` function signature in `src/cache.rs` to accept an optional http_tool parameter:

```rust
pub fn save_trusted_configs(
    alias_name: &str,
    configs: &[(String, String, String)],
    banner: Option<String>,
    http_tool: Option<String>,
) -> Result<(), anyhow::Error>
```

Update the function body to set `http_tool` on the `AliasMetadata` struct before serializing. Update all existing call sites (in `init.rs`, `refresh.rs`, and any tests) to pass `None` for http_tool — preserving current behavior.

- [ ] **Step 5: Run tests**

Run: `eval "$(mise activate zsh)" && cargo test 2>&1`
Expected: All tests pass.

---

## Phase 3: OpenAPI Module

### Task 9: Create openapi module scaffold and HTTP tool detection

**Files:**
- Create: `src/openapi/mod.rs`
- Create: `src/openapi/http_tool.rs`
- Modify: `src/main.rs` (add `mod openapi;`)

- [ ] **Step 1: Create `src/openapi/mod.rs`**

```rust
pub mod http_tool;
pub mod parser;
pub mod transform;
```

- [ ] **Step 2: Write failing test for HTTP tool detection**

In `src/openapi/http_tool.rs`:
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_http_tool_finds_curl_or_wget() {
        // At least one should be available on any dev/CI machine
        let tool = detect_http_tool();
        assert!(tool.is_ok(), "neither curl nor wget found");
        let name = tool.unwrap();
        assert!(name == "curl" || name == "wget", "unexpected tool: {name}");
    }

    #[test]
    fn test_fetch_command_curl() {
        let cmd = build_fetch_command("curl", "https://example.com/spec.json");
        assert!(cmd.contains("curl"));
        assert!(cmd.contains("-s"));
        assert!(cmd.contains("-f"));
        assert!(cmd.contains("-L"));
        assert!(cmd.contains("https://example.com/spec.json"));
    }

    #[test]
    fn test_fetch_command_wget() {
        let cmd = build_fetch_command("wget", "https://example.com/spec.json");
        assert!(cmd.contains("wget"));
        assert!(cmd.contains("-q"));
        assert!(cmd.contains("-O-"));
        assert!(cmd.contains("https://example.com/spec.json"));
    }
}
```

- [ ] **Step 3: Implement HTTP tool detection**

In `src/openapi/http_tool.rs`:
```rust
use std::process::Command;

/// Detect available HTTP tool. Returns "curl" or "wget".
pub fn detect_http_tool() -> Result<String, anyhow::Error> {
    if Command::new("curl").arg("--version").output().map(|o| o.status.success()).unwrap_or(false) {
        return Ok("curl".to_string());
    }
    if Command::new("wget").arg("--version").output().map(|o| o.status.success()).unwrap_or(false) {
        return Ok("wget".to_string());
    }
    anyhow::bail!("curl or wget is required for OpenAPI support. Install one and try again.")
}

/// Build the shell command string to fetch a remote URL.
pub fn build_fetch_command(tool: &str, url: &str) -> String {
    match tool {
        "curl" => format!("curl -s -f -L '{url}'"),
        "wget" => format!("wget -q -O- '{url}'"),
        _ => unreachable!("unsupported tool: {tool}"),
    }
}

/// Fetch a remote URL using the detected tool. Returns the response body.
pub fn fetch_remote(tool: &str, url: &str) -> Result<String, anyhow::Error> {
    let cmd = build_fetch_command(tool, url);
    let output = Command::new("sh")
        .arg("-c")
        .arg(&cmd)
        .output()?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("Failed to download {url}: {stderr}");
    }
    Ok(String::from_utf8(output.stdout)?)
}

/// Generate a curl command string for an API operation.
pub fn generate_curl_command(
    method: &str,
    url: &str,
    headers: &[(String, String)],
    body_json_template: Option<&str>,
) -> String {
    let mut parts = vec![format!("curl -s -X {method} '{url}'")];
    for (key, value) in headers {
        parts.push(format!("-H '{key}: {value}'"));
    }
    if let Some(body) = body_json_template {
        parts.push(format!("-d '{body}'"));
    }
    parts.join(" ")
}

/// Generate a wget command string for an API operation.
pub fn generate_wget_command(
    method: &str,
    url: &str,
    headers: &[(String, String)],
    body_json_template: Option<&str>,
) -> String {
    let mut parts = vec![format!("wget -q -O- --method={method} '{url}'")];
    for (key, value) in headers {
        parts.push(format!("--header='{key}: {value}'"));
    }
    if let Some(body) = body_json_template {
        parts.push(format!("--body-data='{body}'"));
    }
    parts.join(" ")
}
```

- [ ] **Step 4: Add `mod openapi;` to main.rs and create stub files**

Add `mod openapi;` to `src/main.rs`.

Create stub `src/openapi/parser.rs`:
```rust
// OpenAPI spec parsing — implemented in Task 10
```

Create stub `src/openapi/transform.rs`:
```rust
// OpenAPI-to-Configuration transformation — implemented in Task 11+
```

- [ ] **Step 5: Run tests**

Run: `eval "$(mise activate zsh)" && cargo test 2>&1`
Expected: All tests pass, including the 3 new http_tool tests.

---

### Task 10: OpenAPI spec parsing

**Files:**
- Modify: `src/openapi/parser.rs`

- [ ] **Step 1: Add Petstore fixture**

Download or create `tests/fixtures/petstore.json` — the standard Petstore 3.0 spec. Use a minimal version:

```json
{
  "openapi": "3.0.0",
  "info": { "title": "Petstore", "version": "1.0.0" },
  "servers": [{ "url": "https://petstore.example.com/v1" }],
  "paths": {
    "/pets": {
      "get": {
        "operationId": "listPets",
        "summary": "List all pets",
        "parameters": [
          { "name": "limit", "in": "query", "schema": { "type": "integer" } }
        ],
        "responses": { "200": { "description": "A list of pets" } }
      },
      "post": {
        "operationId": "createPet",
        "summary": "Create a pet",
        "requestBody": {
          "required": true,
          "content": {
            "application/json": {
              "schema": {
                "type": "object",
                "required": ["name"],
                "properties": {
                  "name": { "type": "string", "description": "Pet name" },
                  "tag": { "type": "string", "description": "Pet tag" }
                }
              }
            }
          }
        },
        "responses": { "201": { "description": "Pet created" } }
      }
    },
    "/pets/{petId}": {
      "get": {
        "operationId": "getPet",
        "summary": "Get a pet by ID",
        "parameters": [
          { "name": "petId", "in": "path", "required": true, "schema": { "type": "string" } }
        ],
        "responses": { "200": { "description": "A pet" } }
      },
      "delete": {
        "operationId": "deletePet",
        "summary": "Delete a pet",
        "parameters": [
          { "name": "petId", "in": "path", "required": true, "schema": { "type": "string" } }
        ],
        "responses": { "204": { "description": "Pet deleted" } }
      }
    }
  }
}
```

- [ ] **Step 2: Write failing test for spec parsing**

In `src/openapi/parser.rs`:
```rust
use openapiv3::OpenAPI;

/// Parse an OpenAPI spec from a string (JSON or YAML).
pub fn parse_spec(content: &str) -> Result<OpenAPI, anyhow::Error> {
    // Try JSON first, then YAML
    if let Ok(spec) = serde_json::from_str::<OpenAPI>(content) {
        return Ok(spec);
    }
    let spec: OpenAPI = serde_saphyr::from_str(content)
        .map_err(|e| anyhow::anyhow!("Failed to parse OpenAPI spec: {e}"))?;
    Ok(spec)
}

/// Validate that the spec is OpenAPI 3.x (not Swagger 2.0).
pub fn validate_version(spec: &OpenAPI) -> Result<(), anyhow::Error> {
    if !spec.openapi.starts_with("3.") {
        anyhow::bail!(
            "Swagger 2.0 is not yet supported. Please use an OpenAPI 3.0+ spec. Found version: {}",
            spec.openapi
        );
    }
    Ok(())
}

/// Extract the base URL from the first server entry, or return a placeholder.
pub fn extract_base_url(spec: &OpenAPI) -> String {
    spec.servers
        .first()
        .map(|s| s.url.trim_end_matches('/').to_string())
        .unwrap_or_else(|| "http://localhost".to_string())
}

/// Derive a config name from the spec title (slugified).
pub fn derive_config_name(spec: &OpenAPI, fallback_filename: &str) -> String {
    let title = &spec.info.title;
    if title.is_empty() {
        return slugify(fallback_filename);
    }
    slugify(title)
}

fn slugify(s: &str) -> String {
    s.to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_petstore_json() {
        let content = std::fs::read_to_string("tests/fixtures/petstore.json").unwrap();
        let spec = parse_spec(&content).unwrap();
        assert_eq!(spec.openapi, "3.0.0");
        assert_eq!(spec.info.title, "Petstore");
        assert_eq!(spec.paths.paths.len(), 2);
    }

    #[test]
    fn test_validate_version_3() {
        let content = std::fs::read_to_string("tests/fixtures/petstore.json").unwrap();
        let spec = parse_spec(&content).unwrap();
        assert!(validate_version(&spec).is_ok());
    }

    #[test]
    fn test_extract_base_url() {
        let content = std::fs::read_to_string("tests/fixtures/petstore.json").unwrap();
        let spec = parse_spec(&content).unwrap();
        assert_eq!(extract_base_url(&spec), "https://petstore.example.com/v1");
    }

    #[test]
    fn test_derive_config_name() {
        let content = std::fs::read_to_string("tests/fixtures/petstore.json").unwrap();
        let spec = parse_spec(&content).unwrap();
        assert_eq!(derive_config_name(&spec, "fallback"), "petstore");
    }

    #[test]
    fn test_slugify_complex_title() {
        assert_eq!(slugify("My Cool API v2"), "my-cool-api-v2");
        assert_eq!(slugify("  spaces  "), "spaces");
    }

    #[test]
    fn test_reject_swagger_2() {
        let swagger2 = r#"{"swagger": "2.0", "info": {"title": "Old API", "version": "1.0"}, "paths": {}}"#;
        // openapiv3 crate may fail to parse Swagger 2.0, or parse it with version "2.0"
        // Either way, validate_version should reject it
        match parse_spec(swagger2) {
            Ok(spec) => {
                let result = validate_version(&spec);
                assert!(result.is_err(), "should reject Swagger 2.0");
                assert!(result.unwrap_err().to_string().contains("not yet supported"));
            }
            Err(_) => {} // Parsing failure is also acceptable
        }
    }

    #[test]
    fn test_extract_base_url_missing_servers() {
        let no_servers = r#"{"openapi": "3.0.0", "info": {"title": "Test", "version": "1.0"}, "paths": {}}"#;
        let spec = parse_spec(no_servers).unwrap();
        assert_eq!(extract_base_url(&spec), "http://localhost");
    }
}
```

- [ ] **Step 3: Run tests**

Run: `eval "$(mise activate zsh)" && cargo test openapi 2>&1`
Expected: All parser tests pass.

---

### Task 11a: Path parsing, method mapping, and command hierarchy

**Files:**
- Modify: `src/openapi/transform.rs`

**Important: `Command::validate()` interaction**

The existing `Command::validate()` requires every command to have either `cmd` or `subcommands`. OpenAPI-generated intermediate nodes (e.g., `pets` which contains both item operations and nested paths) will only have `subcommands` — this is fine. All flags (including path params like `--pet-id`) must go on the leaf commands that have a `cmd`, not on intermediate container nodes. The transformation must ensure this invariant.

- [ ] **Step 1: Add deep nesting fixture**

Create `tests/fixtures/petstore_nested.json` — extends Petstore with nested paths and request bodies:
```json
{
  "openapi": "3.0.0",
  "info": { "title": "Petstore Nested", "version": "1.0.0" },
  "servers": [{ "url": "https://petstore.example.com/v1" }],
  "paths": {
    "/pets": {
      "get": {
        "operationId": "listPets",
        "summary": "List all pets",
        "parameters": [
          { "name": "limit", "in": "query", "schema": { "type": "integer" } }
        ],
        "responses": { "200": { "description": "A list of pets" } }
      },
      "post": {
        "operationId": "createPet",
        "summary": "Create a pet",
        "requestBody": {
          "required": true,
          "content": {
            "application/json": {
              "schema": {
                "type": "object",
                "required": ["name"],
                "properties": {
                  "name": { "type": "string", "description": "Pet name" },
                  "tag": { "type": "string", "description": "Pet tag" },
                  "owner": {
                    "type": "object",
                    "properties": {
                      "email": { "type": "string", "description": "Owner email" },
                      "address": {
                        "type": "object",
                        "properties": {
                          "city": { "type": "string", "description": "City" },
                          "zip": { "type": "string", "description": "Zip code" }
                        }
                      }
                    }
                  }
                }
              }
            }
          }
        },
        "responses": { "201": { "description": "Pet created" } }
      }
    },
    "/pets/{petId}": {
      "get": {
        "operationId": "getPet",
        "summary": "Get a pet by ID",
        "parameters": [
          { "name": "petId", "in": "path", "required": true, "schema": { "type": "string" } }
        ],
        "responses": { "200": { "description": "A pet" } }
      }
    },
    "/pets/{petId}/toys": {
      "get": {
        "operationId": "listPetToys",
        "summary": "List toys for a pet",
        "parameters": [
          { "name": "petId", "in": "path", "required": true, "schema": { "type": "string" } }
        ],
        "responses": { "200": { "description": "A list of toys" } }
      },
      "post": {
        "operationId": "addPetToy",
        "summary": "Add a toy for a pet",
        "parameters": [
          { "name": "petId", "in": "path", "required": true, "schema": { "type": "string" } }
        ],
        "requestBody": {
          "required": true,
          "content": {
            "application/json": {
              "schema": {
                "type": "object",
                "properties": {
                  "name": { "type": "string", "description": "Toy name" }
                }
              }
            }
          }
        },
        "responses": { "201": { "description": "Toy added" } }
      }
    }
  }
}
```

- [ ] **Step 2: Implement the transformation with tests**

`src/openapi/transform.rs` is the largest piece. It needs:

1. **`transform_spec(spec: &OpenAPI, tool: &str, fallback_name: &str) -> Result<Configuration, anyhow::Error>`** — the public entry point
2. **Path segment parsing** — split `/pets/{petId}/toys` into `["pets", "{petId}", "toys"]`
3. **Command hierarchy building** — group operations by path segments into nested `Command` structs
4. **Method-to-verb mapping** — `GET` collection -> `list`, `GET` item -> `get`, `POST` -> `create`, etc.
5. **Flag generation** — from path params, query params, headers, and request body schemas
6. **Dot-notation body flattening** — recursive schema walking, unlimited depth
7. **curl/wget command string generation** — using `http_tool` functions
8. **JSON body template building** — reconstruct nested JSON from flat flag placeholders

Write tests covering:
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transform_petstore_basic() {
        let content = std::fs::read_to_string("tests/fixtures/petstore.json").unwrap();
        let spec = crate::openapi::parser::parse_spec(&content).unwrap();
        let config = transform_spec(&spec, "curl", "petstore").unwrap();

        assert_eq!(config.name, "petstore");
        assert!(config.commands.contains_key("pets"));
    }

    #[test]
    fn test_collection_vs_item_detection() {
        // /pets -> collection (list, create)
        // /pets/{petId} -> item (get, delete)
        let content = std::fs::read_to_string("tests/fixtures/petstore.json").unwrap();
        let spec = crate::openapi::parser::parse_spec(&content).unwrap();
        let config = transform_spec(&spec, "curl", "petstore").unwrap();

        let pets = config.commands.get("pets").unwrap();
        // pets should have subcommands: list, create (collection) and get, delete (item)
        let subs = pets.subcommands.as_ref().unwrap();
        assert!(subs.contains_key("list"), "missing 'list' subcommand");
        assert!(subs.contains_key("create"), "missing 'create' subcommand");
        assert!(subs.contains_key("get"), "missing 'get' subcommand");
        assert!(subs.contains_key("delete"), "missing 'delete' subcommand");
    }

    #[test]
    fn test_path_params_become_flags() {
        let content = std::fs::read_to_string("tests/fixtures/petstore.json").unwrap();
        let spec = crate::openapi::parser::parse_spec(&content).unwrap();
        let config = transform_spec(&spec, "curl", "petstore").unwrap();

        let pets = config.commands.get("pets").unwrap();
        let get = pets.subcommands.as_ref().unwrap().get("get").unwrap();
        let flag_names: Vec<&str> = get.flags.iter().map(|f| f.name.as_str()).collect();
        assert!(flag_names.contains(&"pet-id"), "missing --pet-id flag: {flag_names:?}");
    }

    #[test]
    fn test_query_params_become_flags() {
        let content = std::fs::read_to_string("tests/fixtures/petstore.json").unwrap();
        let spec = crate::openapi::parser::parse_spec(&content).unwrap();
        let config = transform_spec(&spec, "curl", "petstore").unwrap();

        let pets = config.commands.get("pets").unwrap();
        let list = pets.subcommands.as_ref().unwrap().get("list").unwrap();
        let flag_names: Vec<&str> = list.flags.iter().map(|f| f.name.as_str()).collect();
        assert!(flag_names.contains(&"limit"), "missing --limit flag: {flag_names:?}");
    }

    #[test]
    fn test_nested_path_hierarchy() {
        let content = std::fs::read_to_string("tests/fixtures/petstore_nested.json").unwrap();
        let spec = crate::openapi::parser::parse_spec(&content).unwrap();
        let config = transform_spec(&spec, "curl", "petstore-nested").unwrap();

        // /pets/{petId}/toys -> pets > toys > list/create
        let pets = config.commands.get("pets").unwrap();
        let subs = pets.subcommands.as_ref().unwrap();
        assert!(subs.contains_key("toys"), "missing 'toys' subcommand under pets");
    }

    #[test]
    fn test_dot_notation_body_flags() {
        let content = std::fs::read_to_string("tests/fixtures/petstore_nested.json").unwrap();
        let spec = crate::openapi::parser::parse_spec(&content).unwrap();
        let config = transform_spec(&spec, "curl", "petstore-nested").unwrap();

        let pets = config.commands.get("pets").unwrap();
        let create = pets.subcommands.as_ref().unwrap().get("create").unwrap();
        let flag_names: Vec<&str> = create.flags.iter().map(|f| f.name.as_str()).collect();

        assert!(flag_names.contains(&"name"), "missing --name flag");
        assert!(flag_names.contains(&"tag"), "missing --tag flag");
        // Dot notation for nested — exact format depends on clap compatibility check
        // If clap supports dots: "owner.email", "owner.address.city"
        // If not: "owner-email", "owner-address-city"
        let has_nested = flag_names.iter().any(|n| n.contains("owner") && n.contains("email"));
        assert!(has_nested, "missing nested owner email flag: {flag_names:?}");
    }

    #[test]
    fn test_generated_curl_command() {
        let content = std::fs::read_to_string("tests/fixtures/petstore.json").unwrap();
        let spec = crate::openapi::parser::parse_spec(&content).unwrap();
        let config = transform_spec(&spec, "curl", "petstore").unwrap();

        let pets = config.commands.get("pets").unwrap();
        let list = pets.subcommands.as_ref().unwrap().get("list").unwrap();
        let cmd = list.cmd.as_ref().unwrap();
        let run_cmd = &cmd.run[0];
        assert!(run_cmd.contains("curl"), "expected curl command: {run_cmd}");
        assert!(run_cmd.contains("GET"), "expected GET method: {run_cmd}");
        assert!(run_cmd.contains("petstore.example.com"), "expected base URL: {run_cmd}");
    }

    #[test]
    fn test_generated_wget_command() {
        let content = std::fs::read_to_string("tests/fixtures/petstore.json").unwrap();
        let spec = crate::openapi::parser::parse_spec(&content).unwrap();
        let config = transform_spec(&spec, "wget", "petstore").unwrap();

        let pets = config.commands.get("pets").unwrap();
        let list = pets.subcommands.as_ref().unwrap().get("list").unwrap();
        let cmd = list.cmd.as_ref().unwrap();
        let run_cmd = &cmd.run[0];
        assert!(run_cmd.contains("wget"), "expected wget command: {run_cmd}");
    }
}
```

Implement these functions for Task 11a:
- `fn method_to_verb(method: &str, is_item: bool) -> &str`
- `fn path_to_segments(path: &str) -> Vec<PathSegment>` (where PathSegment is either Literal(String) or Param(String))
- `fn build_command_for_operation(method, path, operation, base_url, tool) -> (Vec<String>, Command)` (returns path segments and the command) — for now, only handle path params and query params as flags, skip request body
- `fn merge_operations_into_hierarchy(operations) -> HashMap<String, Command>` (group by path segments into nested Commands)
- `fn transform_spec(spec, tool, fallback_name) -> Result<Configuration, anyhow::Error>` (orchestrator)

Remember: intermediate hierarchy nodes get `subcommands` only (no `cmd`). All flags go on leaf commands. Path parameters like `--pet-id` are placed on every leaf command that needs them, not on container nodes.

- [ ] **Step 3: Verify clap accepts dot-notation in flag names**

Write a quick unit test that creates a clap `Arg` with `.long("owner.email")` and verify it works. If clap rejects dots, use hyphens (`owner-email`) and document the mapping. This determines the flag naming strategy for Task 11b.

- [ ] **Step 4: Run all tests**

Run: `eval "$(mise activate zsh)" && cargo test 2>&1`
Expected: All tests pass.

---

### Task 11b: Schema flattening and JSON body template generation

**Files:**
- Modify: `src/openapi/transform.rs`

- [ ] **Step 1: Write failing tests for dot-notation body flattening**

```rust
#[test]
fn test_dot_notation_body_flags_deep() {
    let content = std::fs::read_to_string("tests/fixtures/petstore_nested.json").unwrap();
    let spec = crate::openapi::parser::parse_spec(&content).unwrap();
    let config = transform_spec(&spec, "curl", "petstore-nested").unwrap();

    let pets = config.commands.get("pets").unwrap();
    let create = pets.subcommands.as_ref().unwrap().get("create").unwrap();
    let flag_names: Vec<&str> = create.flags.iter().map(|f| f.name.as_str()).collect();

    assert!(flag_names.contains(&"name"), "missing --name flag");
    assert!(flag_names.contains(&"tag"), "missing --tag flag");
    // Verify deep nesting (3 levels: owner > address > city)
    let has_city = flag_names.iter().any(|n| n.contains("owner") && n.contains("address") && n.contains("city"));
    assert!(has_city, "missing deeply nested city flag: {flag_names:?}");
}

#[test]
fn test_json_body_template_reconstructs_nesting() {
    // The generated curl -d '...' should contain proper nested JSON
    let content = std::fs::read_to_string("tests/fixtures/petstore_nested.json").unwrap();
    let spec = crate::openapi::parser::parse_spec(&content).unwrap();
    let config = transform_spec(&spec, "curl", "petstore-nested").unwrap();

    let pets = config.commands.get("pets").unwrap();
    let create = pets.subcommands.as_ref().unwrap().get("create").unwrap();
    let cmd_str = &create.cmd.as_ref().unwrap().run[0];

    // Should contain nested JSON structure with placeholders
    assert!(cmd_str.contains("owner"), "missing owner in body: {cmd_str}");
    assert!(cmd_str.contains("address"), "missing address in body: {cmd_str}");
}
```

- [ ] **Step 2: Implement schema flattening and JSON body generation**

Add to `transform.rs`:
- `fn flatten_schema_to_flags(schema: &Schema, prefix: &str) -> Vec<Flag>` — recursive, uses dot-notation (or hyphens per Step 3 of Task 11a), unlimited depth
- `fn build_json_template(flags: &[Flag]) -> String` — reconstructs nested JSON from flat flag names using `${{flag_name}}` placeholders

Wire body flag generation into `build_command_for_operation` (which previously only handled path/query params).

- [ ] **Step 3: Run all tests**

Run: `eval "$(mise activate zsh)" && cargo test 2>&1`
Expected: All tests pass.

---

### Task 12: OpenAPI security scheme handling

**Files:**
- Modify: `src/openapi/transform.rs`

- [ ] **Step 1: Add fixture with security schemes**

Create `tests/fixtures/petstore_auth.json` — extends Petstore with security:
```json
{
  "openapi": "3.0.0",
  "info": { "title": "Petstore Auth", "version": "1.0.0" },
  "servers": [{ "url": "https://petstore.example.com/v1" }],
  "components": {
    "securitySchemes": {
      "bearerAuth": {
        "type": "http",
        "scheme": "bearer"
      },
      "apiKey": {
        "type": "apiKey",
        "in": "header",
        "name": "X-API-Key"
      }
    }
  },
  "security": [{ "bearerAuth": [] }],
  "paths": {
    "/pets": {
      "get": {
        "operationId": "listPets",
        "summary": "List all pets",
        "responses": { "200": { "description": "A list of pets" } }
      }
    }
  }
}
```

- [ ] **Step 2: Write failing tests for auth**

```rust
#[test]
fn test_bearer_auth_generates_env_var_header() {
    let content = std::fs::read_to_string("tests/fixtures/petstore_auth.json").unwrap();
    let spec = crate::openapi::parser::parse_spec(&content).unwrap();
    let config = transform_spec(&spec, "curl", "petstore-auth").unwrap();

    let pets = config.commands.get("pets").unwrap();
    let list = pets.subcommands.as_ref().unwrap().get("list").unwrap();
    let cmd_str = &list.cmd.as_ref().unwrap().run[0];
    assert!(cmd_str.contains("Authorization: Bearer"), "missing auth header: {cmd_str}");
    assert!(cmd_str.contains("${{env."), "missing env var placeholder: {cmd_str}");
}

#[test]
fn test_security_scheme_to_env_var_name() {
    assert_eq!(security_scheme_to_env_var("bearerAuth"), "BEARER_AUTH_TOKEN");
    assert_eq!(security_scheme_to_env_var("apiKey"), "API_KEY_TOKEN");
}
```

- [ ] **Step 3: Implement security scheme handling**

Add to `transform.rs`:
- `fn security_scheme_to_env_var(scheme_name: &str) -> String` — convert camelCase to UPPER_SNAKE_CASE, append `_TOKEN`
- `fn extract_auth_headers(spec: &OpenAPI) -> Vec<(String, String)>` — read security schemes, generate `Authorization: Bearer ${{env.VAR}}` or `X-API-Key: ${{env.VAR}}` headers

Wire auth headers into the curl/wget command generation.

- [ ] **Step 4: Run tests**

Run: `eval "$(mise activate zsh)" && cargo test 2>&1`
Expected: All tests pass.

---

### Task 13: Unsupported features handling

**Files:**
- Modify: `src/openapi/transform.rs`

- [ ] **Step 1: Add fixture with unsupported features**

Create `tests/fixtures/openapi_mixed.json` with:
- A normal GET/POST endpoint (should work)
- An endpoint with XML-only response (should skip)
- An endpoint with `oneOf` schema in request body (should best-effort)
- An endpoint with `allOf` schema in request body (should merge)
- Include a `"x-websocket": true` or similar marker to test websocket skip

- [ ] **Step 2: Write tests for skip/warn behavior**

```rust
#[test]
fn test_unsupported_operations_skipped_with_warnings() {
    let content = std::fs::read_to_string("tests/fixtures/openapi_mixed.json").unwrap();
    let spec = crate::openapi::parser::parse_spec(&content).unwrap();
    let mut warnings = Vec::new();
    let config = transform_spec_with_warnings(&spec, "curl", "mixed", &mut warnings).unwrap();

    // Normal operations should be present
    assert!(!config.commands.is_empty());
    // Warnings should be collected
    assert!(!warnings.is_empty());
    assert!(warnings.iter().any(|w| w.contains("skip") || w.contains("Skip")));
}

#[test]
fn test_allof_schemas_merged() {
    let content = std::fs::read_to_string("tests/fixtures/openapi_mixed.json").unwrap();
    let spec = crate::openapi::parser::parse_spec(&content).unwrap();
    let mut warnings = Vec::new();
    let config = transform_spec_with_warnings(&spec, "curl", "mixed", &mut warnings).unwrap();

    // allOf endpoint should produce merged flags from all schemas
    // The specific assertions depend on the fixture content
    // At minimum: no warnings about allOf being unsupported
    assert!(!warnings.iter().any(|w| w.contains("allOf") && w.contains("skip")),
        "allOf should be merged, not skipped");
}
```

- [ ] **Step 3: Add warnings collection to transform**

Modify `transform_spec` to accept `&mut Vec<String>` for warnings (or add a `transform_spec_with_warnings` variant). Detect:
- XML-only content types -> skip with warning
- Websocket-style operations -> skip with warning
- `oneOf`/`anyOf` schemas -> best-effort (generate flags for all variants), add warning
- `allOf` schemas -> merge all sub-schemas into one, generate combined flags, proceed without warning
- File uploads (multipart/form-data) -> best-effort (generate `--file` flag), add warning

Also add `fn summarize_warnings(warnings: &[String]) -> String` that produces the init summary:
```
Generated N commands from OpenAPI spec
Skipped M operations (reasons)
K operations use best-effort approximations (use --verbose for details)
```

- [ ] **Step 4: Run tests**

Run: `eval "$(mise activate zsh)" && cargo test 2>&1`
Expected: All tests pass.

---

### Task 14: Wire OpenAPI into init flow

**Files:**
- Modify: `src/init.rs`
- Modify: `src/openapi/mod.rs`

- [ ] **Step 1: Add public API to `src/openapi/mod.rs`**

```rust
pub mod http_tool;
pub mod parser;
pub mod transform;

use crate::models::Configuration;

/// Process an OpenAPI config path. Returns (config_name, source_identifier, yaml_content, raw_source_for_hashing).
pub fn process_openapi_path(
    path: &str,
    tool: &str,
    yes: bool,
    verbose: bool,
) -> Result<(Configuration, String), anyhow::Error> {
    let is_remote = path.starts_with("http://") || path.starts_with("https://");

    let raw_content = if is_remote {
        if !yes {
            eprint!("Warning: ring-cli will use '{}' to download {}\nContinue? [Y/n] ", tool, path);
            let mut input = String::new();
            std::io::stdin().read_line(&mut input)?;
            if input.trim().to_lowercase() == "n" {
                anyhow::bail!("Download cancelled by user");
            }
        }
        eprintln!("Downloading OpenAPI spec...");
        http_tool::fetch_remote(tool, path)?
    } else {
        std::fs::read_to_string(path)
            .map_err(|e| anyhow::anyhow!("Failed to read OpenAPI spec at {path}: {e}"))?
    };

    let spec = parser::parse_spec(&raw_content)?;
    parser::validate_version(&spec)?;

    let fallback_name = std::path::Path::new(path)
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "api".to_string());

    let mut warnings = Vec::new();
    let config = transform::transform_spec_with_warnings(&spec, tool, &fallback_name, &mut warnings)?;

    for w in &warnings {
        if verbose {
            eprintln!("{}", w);
        }
    }

    let summary = transform::summarize_warnings(&warnings);
    if !summary.is_empty() {
        eprintln!("{}", summary);
    }

    Ok((config, raw_content))
}
```

- [ ] **Step 2: Update `init.rs` to handle `openapi:` prefix**

In `handle_init`, when iterating over config paths:
- Check if path starts with `openapi:`
- Strip prefix, detect HTTP tool (once), call `openapi::process_openapi_path()`
- Serialize resulting `Configuration` to YAML for caching
- Use raw OpenAPI content for hashing
- Store `openapi:` prefix in source_path for refresh awareness

- [ ] **Step 3: Write integration test**

In `tests/integration.rs`:
```rust
#[test]
fn test_init_openapi_local_spec() {
    let output = cargo_bin()
        .args([
            "init",
            "--config-path", "openapi:tests/fixtures/petstore.json",
            "--alias", "petstore-test",
            "--force",
            "--yes",
        ])
        .output()
        .expect("failed to run");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "init with OpenAPI spec failed:\nstdout: {stdout}\nstderr: {stderr}"
    );

    // Verify cached config files exist
    let alias_dir = dirs::home_dir().unwrap().join(".ring-cli/aliases/petstore-test");
    assert!(alias_dir.join("metadata.json").exists(), "metadata.json should exist");
    // Config file should be named after the spec's info.title (slugified)
    assert!(alias_dir.join("petstore.yml").exists(), "petstore.yml cache should exist");

    // Verify the alias --help shows expected subcommands
    let help_output = cargo_bin()
        .args(["--alias-mode", "petstore-test", "petstore", "--help"])
        .output()
        .expect("failed to run alias help");
    let help_stdout = String::from_utf8_lossy(&help_output.stdout);
    let help_combined = format!("{}{}", help_stdout, String::from_utf8_lossy(&help_output.stderr));
    assert!(help_combined.contains("pets"), "help should show 'pets' subcommand: {help_combined}");
}
```

- [ ] **Step 4: Run tests**

Run: `eval "$(mise activate zsh)" && cargo test 2>&1`
Expected: All tests pass.

---

### Task 15: Wire OpenAPI into refresh flow and add `--yes` to refresh

**Files:**
- Modify: `src/refresh.rs`
- Modify: `src/cli.rs` (add `--yes` to refresh-configuration)
- Modify: `src/main.rs` (pass `--yes` to refresh)

- [ ] **Step 0: Add `--yes` flag to `refresh-configuration` command**

In `src/cli.rs`, in the `build_cli` function where `refresh-configuration` is defined as a subcommand, add:
```rust
.arg(
    clap::Arg::new("yes")
        .long("yes")
        .help("Skip confirmation prompts (for CI/automation)")
        .action(clap::ArgAction::SetTrue),
)
```

Update `handle_refresh_configuration` and `handle_check_updates` in `refresh.rs` to accept a `yes: bool` parameter. When `yes` is true, skip both download consent and trust confirmation prompts.

- [ ] **Step 1: Update refresh to handle `openapi:` source paths**

In `handle_refresh_configuration` and `handle_check_updates`, when iterating over `ConfigEntry` items:
- Check if `entry.source_path` starts with `openapi:`
- If so, strip prefix, determine if local/remote
- For remote: re-fetch using stored `http_tool` from metadata (validate it exists, fallback to re-detect)
- Parse and transform the spec
- Hash the raw spec content
- Compare with stored hash
- If changed: prompt to trust (or auto-accept with `--yes`)

- [ ] **Step 2: Update `resolve_references` for openapi: entries**

In `init.rs`, update `resolve_references()` to skip filesystem path joining and `.exists()` checks for entries starting with `openapi:`.

- [ ] **Step 3: Write integration test for refresh with OpenAPI**

```rust
#[test]
fn test_refresh_openapi_no_changes() {
    // Init with OpenAPI spec
    let output = cargo_bin()
        .args([
            "init",
            "--config-path", "openapi:tests/fixtures/petstore.json",
            "--alias", "refresh-oa-test",
            "--force", "--yes",
        ])
        .output()
        .expect("init failed");
    assert!(output.status.success());

    // Refresh should detect no changes
    let refresh = cargo_bin()
        .args(["--alias-mode", "refresh-oa-test", "refresh-configuration"])
        .output()
        .expect("refresh failed");
    let stderr = String::from_utf8_lossy(&refresh.stderr);
    assert!(
        refresh.status.success(),
        "refresh failed: {stderr}"
    );
}
```

- [ ] **Step 4: Run tests**

Run: `eval "$(mise activate zsh)" && cargo test 2>&1`
Expected: All tests pass.

---

## Phase 4: Documentation & README

### Task 16: Move current README to getting-started.md

**Files:**
- Create: `docs/getting-started.md`
- Read: `README.md` (current content)

- [ ] **Step 1: Copy current README.md content to `docs/getting-started.md`**

Read the current `README.md` and write its contents to `docs/getting-started.md`. Add a note at the top: "# Getting Started Guide" and update any relative links.

- [ ] **Step 2: Verify the file is correct**

Read `docs/getting-started.md` to confirm contents match.

---

### Task 17: Write new marketing README

**Files:**
- Modify: `README.md`

- [ ] **Step 1: Write new README.md**

Follow the structure from the design spec Section 7. The README should:
- Open with a clear tagline
- Show value proposition for devs, devops, and AI agents
- Include install oneliner + alternatives
- Quick Start for both YAML and OpenAPI
- Feature list with brief descriptions
- Link to docs
- No emojis, no buzzwords, technical but approachable
- All code examples must be real, runnable commands

---

### Task 18: Write OpenAPI guide

**Files:**
- Create: `docs/openapi-guide.md`

- [ ] **Step 1: Write the OpenAPI usage guide**

Cover:
- What is OpenAPI support in ring-cli
- Local spec: `ring-cli init --config-path openapi:./spec.yml --alias myapi`
- Remote spec: `ring-cli init --config-path openapi:https://... --alias myapi --yes`
- How paths map to commands (with examples)
- How request bodies become flags (dot-notation)
- Authentication via environment variables
- Refreshing OpenAPI specs
- Mixing OpenAPI and regular YAML configs
- Known limitations (unsupported features list)
- Troubleshooting (curl/wget not found, invalid spec, etc.)

---

### Task 19: Write configuration reference

**Files:**
- Create: `docs/configuration-reference.md`

- [ ] **Step 1: Write the YAML schema reference**

Cover:
- Complete YAML schema with all fields documented
- Config fields table (version, name, description, base-dir, banner, commands)
- Command fields table (description, flags, cmd, subcommands)
- Flag fields table (name, short, description)
- Variable substitution syntax (`${{flag}}`, `${{env.VAR}}`)
- Validation rules
- Multi-config composition
- References file format
- Examples for common patterns

---

### Task 20: Create install.sh

**Files:**
- Create: `install.sh`

- [ ] **Step 1: Write the install script**

```bash
#!/bin/sh
set -e

REPO="MichaelCereda/ring-cli"
INSTALL_DIR="${INSTALL_DIR:-$HOME/.local/bin}"

# Detect OS
OS="$(uname -s)"
case "$OS" in
    Linux)  OS_NAME="Linux" ;;
    Darwin) OS_NAME="Darwin" ;;
    *)      echo "Error: Unsupported OS: $OS"; exit 1 ;;
esac

# Detect architecture
ARCH="$(uname -m)"
case "$ARCH" in
    x86_64|amd64)  ARCH_NAME="x86_64" ;;
    aarch64|arm64) ARCH_NAME="aarch64" ;;
    armv7l)        ARCH_NAME="arm" ;;
    *)             echo "Error: Unsupported architecture: $ARCH"; exit 1 ;;
esac

# Determine archive name
if [ "$OS_NAME" = "Linux" ]; then
    ARCHIVE="ring-cli-${OS_NAME}-${ARCH_NAME}-musl.tar.gz"
else
    ARCHIVE="ring-cli-${OS_NAME}-${ARCH_NAME}.tar.gz"
fi

# Get latest release tag
LATEST=$(curl -s "https://api.github.com/repos/${REPO}/releases/latest" | grep '"tag_name"' | cut -d'"' -f4)
if [ -z "$LATEST" ]; then
    # Fallback to wget
    LATEST=$(wget -q -O- "https://api.github.com/repos/${REPO}/releases/latest" | grep '"tag_name"' | cut -d'"' -f4)
fi

if [ -z "$LATEST" ]; then
    echo "Error: Could not determine latest release version"
    exit 1
fi

URL="https://github.com/${REPO}/releases/download/${LATEST}/${ARCHIVE}"
echo "Downloading ring-cli ${LATEST} for ${OS_NAME}-${ARCH_NAME}..."

# Download and extract
mkdir -p "$INSTALL_DIR"
if command -v curl >/dev/null 2>&1; then
    curl -fsSL "$URL" | tar xz -C "$INSTALL_DIR"
elif command -v wget >/dev/null 2>&1; then
    wget -q -O- "$URL" | tar xz -C "$INSTALL_DIR"
else
    echo "Error: curl or wget is required"
    exit 1
fi

echo "Installed ring-cli to ${INSTALL_DIR}/ring-cli"

# Check if in PATH
case ":$PATH:" in
    *":${INSTALL_DIR}:"*) ;;
    *) echo "Add ${INSTALL_DIR} to your PATH: export PATH=\"${INSTALL_DIR}:\$PATH\"" ;;
esac

echo "Run 'ring-cli --help' to get started."
```

- [ ] **Step 2: Make it executable**

Run: `chmod +x install.sh`

---

### Task 21: Update AGENTS.md and setup-guide.md

**Files:**
- Modify: `AGENTS.md`
- Modify: `docs/setup-guide.md`

- [ ] **Step 1: Update AGENTS.md project structure**

Update the project structure section to reflect all new files (shell.rs, init.rs, refresh.rs, config.rs, openapi/).

- [ ] **Step 2: Update setup-guide.md**

Add OpenAPI section to the existing setup guide. Update any outdated references.

---

## Phase 5: Final Verification

### Task 22: Full test suite and cleanup

- [ ] **Step 1: Run full test suite**

Run: `eval "$(mise activate zsh)" && cargo test 2>&1`
Expected: All tests pass (original 43 + new OpenAPI tests).

- [ ] **Step 2: Run clippy**

Run: `eval "$(mise activate zsh)" && cargo clippy 2>&1`
Expected: No warnings.

- [ ] **Step 3: Verify binary builds in release mode**

Run: `eval "$(mise activate zsh)" && cargo build --release 2>&1`
Expected: Clean build.

- [ ] **Step 4: Manual smoke test**

```bash
# Test YAML config (existing behavior)
eval "$(mise activate zsh)" && cargo run -- init --config-path tests/fixtures/valid_config.yml --alias smoke-test --force
# Test OpenAPI config
eval "$(mise activate zsh)" && cargo run -- init --config-path openapi:tests/fixtures/petstore.json --alias petstore-smoke --force --yes
```

- [ ] **Step 5: Verify no emojis in output**

Run the smoke tests and verify all stderr/stdout output is ASCII-only.
