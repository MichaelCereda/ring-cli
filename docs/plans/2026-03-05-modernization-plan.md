# Ring-CLI Modernization Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Modernize ring-cli dependencies (fixing 12 CVEs), rewrite CLI for clap v4, add env var support, better errors, init command, and a full test suite.

**Architecture:** Keep the existing 4-module structure (main, cli, models, utils) and add an errors module. Commands remain dynamic (built from YAML at runtime) using clap v4's builder API. Drop OpenSSL for rustls-tls everywhere.

**Tech Stack:** Rust 2021, clap 4.5, serde_yml 0.0.12, reqwest 0.13, tokio 1.50, thiserror 2, anyhow 1

**Build command:** `eval "$(mise activate zsh)" && cargo build`
**Test command:** `eval "$(mise activate zsh)" && cargo test`

---

### Task 1: Update Cargo.toml Dependencies

**Files:**
- Modify: `Cargo.toml`

**Step 1: Replace Cargo.toml with modernized dependencies**

```toml
[package]
name = "ring-cli"
version = "1.1.0"
edition = "2021"

[dependencies]
clap = "4.5"
serde = { version = "1.0", features = ["derive"] }
serde_yml = "0.0.12"
reqwest = { version = "0.13", default-features = false, features = ["gzip", "json", "rustls-tls"] }
tokio = { version = "1", features = ["rt-multi-thread", "macros"] }
dirs = "6.0"
thiserror = "2.0"
anyhow = "1.0"
```

No more platform-specific sections. No more openssl. No more serde_derive.

**Step 2: Delete Cargo.lock so it regenerates**

Run: `rm Cargo.lock`

**Step 3: Verify it downloads and resolves (will NOT compile yet — source code still uses old APIs)**

Run: `eval "$(mise activate zsh)" && cargo check 2>&1 | head -30`

Expected: Compilation errors in source files (clap v2 API, serde_yaml imports). This is expected — we'll fix the source next.

**Step 4: Commit**

```bash
git add Cargo.toml
git commit -m "chore: modernize dependencies in Cargo.toml

Upgrade clap 2.33 -> 4.5, reqwest 0.11 -> 0.13, tokio full -> minimal,
dirs 3 -> 6. Replace serde_yaml with serde_yml. Remove serde_derive
(use serde derive feature). Drop vendored OpenSSL for rustls-tls.
Add thiserror and anyhow for error handling."
```

---

### Task 2: Create Error Types Module

**Files:**
- Create: `src/errors.rs`

**Step 1: Create `src/errors.rs`**

```rust
use thiserror::Error;

#[derive(Debug, Error)]
pub enum RingError {
    #[error("Config error: {0}")]
    Config(String),

    #[error("Error parsing '{path}': {source}")]
    YamlParse {
        path: String,
        source: serde_yml::Error,
    },

    #[error("IO error reading '{path}': {source}")]
    Io {
        path: String,
        source: std::io::Error,
    },

    #[error("{context}: {message}")]
    Validation { context: String, message: String },

    #[error("Command '{command}' failed with exit code {code}: {stderr}")]
    ShellCommand {
        command: String,
        code: i32,
        stderr: String,
    },

    #[error("{method} {url}: {message}")]
    Http {
        method: String,
        url: String,
        message: String,
    },

    #[error("Environment variable '{name}' is not set")]
    EnvVar { name: String },

    #[error("Unsupported HTTP method '{0}'")]
    UnsupportedMethod(String),
}
```

**Step 2: Commit**

```bash
git add src/errors.rs
git commit -m "feat: add error types module with thiserror"
```

---

### Task 3: Update Models Module

**Files:**
- Modify: `src/models.rs`

**Step 1: Rewrite `src/models.rs`**

Changes from current:
- `serde_derive::{Deserialize, Serialize}` → `serde::{Deserialize, Serialize}` (derive feature)
- `Command::validate` now takes a `context` parameter for error paths
- Returns `RingError::Validation` instead of plain strings

```rust
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::errors::RingError;

#[derive(Debug, Deserialize, Serialize)]
pub struct Configuration {
    pub version: String,
    pub description: String,
    pub slug: String,
    pub commands: HashMap<String, Command>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Command {
    pub description: String,
    #[serde(default)]
    pub flags: Vec<Flag>,
    pub cmd: Option<CmdType>,
    pub subcommands: Option<HashMap<String, Command>>,
}

impl Command {
    pub fn validate(&self, context: &str) -> Result<(), RingError> {
        match (&self.cmd, &self.subcommands) {
            (Some(_), Some(_)) => {
                return Err(RingError::Validation {
                    context: context.to_string(),
                    message: "Only 'cmd' or 'subcommands' should be present, not both.".to_string(),
                })
            }
            (None, None) => {
                return Err(RingError::Validation {
                    context: context.to_string(),
                    message: "Either 'cmd' or 'subcommands' must be present.".to_string(),
                })
            }
            _ => (),
        }

        if let Some(subcommands) = &self.subcommands {
            for (sub_name, sub_cmd) in subcommands {
                sub_cmd.validate(&format!("{} > {}", context, sub_name))?;
            }
        }

        Ok(())
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Flag {
    pub name: String,
    #[serde(default)]
    pub short: Option<String>,
    pub description: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub enum CmdType {
    Http { http: Http },
    Run { run: Vec<String> },
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Http {
    pub method: String,
    pub url: String,
    pub headers: Option<HashMap<String, String>>,
    #[serde(default)]
    pub body: Option<String>,
}
```

**Step 2: Commit**

```bash
git add src/models.rs
git commit -m "refactor: update models to use serde derive feature and contextual errors"
```

---

### Task 4: Update Utils Module

**Files:**
- Modify: `src/utils.rs`

**Step 1: Rewrite `src/utils.rs`**

Changes from current:
- `serde_yaml` → `serde_yml`
- `dirs` v3 API → v6 (same API, just version bump)
- `replace_placeholders` now takes a `HashMap<String, String>` instead of clap internals
- New `replace_env_vars` function for `${{env.XXX}}`
- Returns `RingError` instead of `Box<dyn Error>`
- Config loading shows file path in errors

```rust
use std::collections::HashMap;
use std::fs;

use crate::errors::RingError;
use crate::models::Configuration;

/// Replace ${{flag_name}} placeholders with flag values from the provided map.
pub fn replace_placeholders(
    template: &str,
    flag_values: &HashMap<String, String>,
    verbose: bool,
) -> String {
    let mut result = template.to_string();
    for (flag_name, flag_value) in flag_values {
        if verbose {
            println!("Replacing placeholder for {}: {}", flag_name, flag_value);
        }
        result = result.replace(&format!("${{{{{}}}}}", flag_name), flag_value);
    }
    result
}

/// Replace ${{env.VAR_NAME}} placeholders with environment variable values.
pub fn replace_env_vars(template: &str, verbose: bool) -> Result<String, RingError> {
    let mut result = template.to_string();
    while let Some(start) = result.find("${{env.") {
        let rest = &result[start + 7..];
        let end = rest.find("}}").ok_or_else(|| RingError::Config(
            format!("Unclosed placeholder starting at position {}", start),
        ))?;
        let var_name = &rest[..end];
        let var_value = std::env::var(var_name).map_err(|_| RingError::EnvVar {
            name: var_name.to_string(),
        })?;
        if verbose {
            println!("Replacing env var {}: {}", var_name, var_value);
        }
        result = result.replace(&format!("${{{{env.{}}}}}", var_name), &var_value);
    }
    Ok(result)
}

pub fn load_configurations(
    config_path: Option<&str>,
) -> Result<Vec<Configuration>, RingError> {
    let mut configurations = Vec::new();

    let default_config_dir = dirs::home_dir()
        .ok_or_else(|| RingError::Config("Unable to determine home directory".to_string()))?
        .join(".ring-cli/configurations");

    let config_dir = if let Some(path) = config_path {
        std::path::PathBuf::from(path)
    } else {
        default_config_dir
    };

    if config_dir.is_file() {
        let path_str = config_dir.display().to_string();
        let content = fs::read_to_string(&config_dir).map_err(|e| RingError::Io {
            path: path_str.clone(),
            source: e,
        })?;
        let config: Configuration = serde_yml::from_str(&content).map_err(|e| {
            RingError::YamlParse {
                path: path_str,
                source: e,
            }
        })?;
        configurations.push(config);
    } else if config_dir.is_dir() {
        let paths = fs::read_dir(&config_dir).map_err(|e| RingError::Io {
            path: config_dir.display().to_string(),
            source: e,
        })?;
        for entry in paths {
            let entry = entry.map_err(|e| RingError::Io {
                path: config_dir.display().to_string(),
                source: e,
            })?;
            let path = entry.path();
            // Only process .yml and .yaml files
            match path.extension().and_then(|e| e.to_str()) {
                Some("yml") | Some("yaml") => {}
                _ => continue,
            }
            let path_str = path.display().to_string();
            let content = fs::read_to_string(&path).map_err(|e| RingError::Io {
                path: path_str.clone(),
                source: e,
            })?;
            let config: Configuration =
                serde_yml::from_str(&content).map_err(|e| RingError::YamlParse {
                    path: path_str,
                    source: e,
                })?;
            configurations.push(config);
        }
    } else {
        return Err(RingError::Config(format!(
            "Config path '{}' is neither a file nor a directory",
            config_dir.display()
        )));
    }

    for config in &configurations {
        for (cmd_name, cmd) in &config.commands {
            cmd.validate(&format!("{} > {}", config.slug, cmd_name))?;
        }
    }

    Ok(configurations)
}
```

**Step 2: Commit**

```bash
git add src/utils.rs
git commit -m "refactor: update utils for serde_yml, env var placeholders, and better errors"
```

---

### Task 5: Rewrite CLI Module

**Files:**
- Modify: `src/cli.rs`

**Step 1: Rewrite `src/cli.rs` for clap v4**

Key changes:
- `clap::App` → `clap::Command`
- `clap::SubCommand::with_name()` → `clap::Command::new()`
- `Arg::with_name().short("x")` → `Arg::new().short('x')`
- `matches.value_of()` → `matches.get_one::<String>()`
- `matches.get_flag()` for boolean flags
- `replace_placeholders` now takes `HashMap<String, String>` — extract flag values from matches using known flag names from the Command model
- `execute_http_request` also uses the HashMap approach
- All functions return `RingError`

```rust
use std::collections::HashMap;
use std::process::Command as ShellCommand;

use crate::errors::RingError;
use crate::models::{CmdType, Command as RingCommand, Configuration, Http};
use crate::utils::{replace_env_vars, replace_placeholders};

fn extract_flag_values(
    flags: &[crate::models::Flag],
    matches: &clap::ArgMatches,
) -> HashMap<String, String> {
    let mut values = HashMap::new();
    for flag in flags {
        if let Some(val) = matches.get_one::<String>(&flag.name) {
            values.insert(flag.name.clone(), val.clone());
        }
    }
    values
}

pub fn add_subcommands_to_cli(
    command: &RingCommand,
    cmd_subcommand: clap::Command,
) -> clap::Command {
    let mut updated_subcommand = cmd_subcommand;
    if let Some(subcommands) = &command.subcommands {
        for (sub_name, sub_cmd) in subcommands {
            let mut sub_cli =
                clap::Command::new(sub_name.clone()).about(sub_cmd.description.clone());
            for flag in &sub_cmd.flags {
                let mut arg = clap::Arg::new(&flag.name)
                    .long(&flag.name)
                    .help(&flag.description);
                if let Some(short_form) = &flag.short {
                    if let Some(c) = short_form.chars().next() {
                        arg = arg.short(c);
                    }
                }
                sub_cli = sub_cli.arg(arg);
            }
            sub_cli = add_subcommands_to_cli(sub_cmd, sub_cli);
            updated_subcommand = updated_subcommand.subcommand(sub_cli);
        }
    }
    updated_subcommand
}

pub fn build_cli_from_configs(configs: &[Configuration]) -> clap::Command {
    let mut app = clap::Command::new("ring-cli")
        .version("1.1.0")
        .about("Ring CLI Tool powered by YAML configurations")
        .arg(
            clap::Arg::new("quiet")
                .short('q')
                .long("quiet")
                .help("Suppress error messages")
                .action(clap::ArgAction::SetTrue),
        )
        .arg(
            clap::Arg::new("verbose")
                .short('v')
                .long("verbose")
                .help("Print verbose output")
                .action(clap::ArgAction::SetTrue),
        )
        .arg(
            clap::Arg::new("config")
                .short('c')
                .long("config")
                .value_name("PATH")
                .help("Path to a custom configuration file or directory"),
        )
        .arg(
            clap::Arg::new("base_dir")
                .short('b')
                .long("base-dir")
                .value_name("PATH")
                .help("Base directory for relative paths"),
        );
    for config in configs {
        let mut subcommand = clap::Command::new(config.slug.clone())
            .about(config.description.clone())
            .version(config.version.clone());
        for (cmd_name, cmd) in &config.commands {
            let mut cmd_subcommand =
                clap::Command::new(cmd_name.clone()).about(cmd.description.clone());
            for flag in &cmd.flags {
                let mut arg = clap::Arg::new(&flag.name)
                    .long(&flag.name)
                    .help(&flag.description);
                if let Some(short_form) = &flag.short {
                    if let Some(c) = short_form.chars().next() {
                        arg = arg.short(c);
                    }
                }
                cmd_subcommand = cmd_subcommand.arg(arg);
            }
            cmd_subcommand = add_subcommands_to_cli(cmd, cmd_subcommand);
            subcommand = subcommand.subcommand(cmd_subcommand);
        }
        app = app.subcommand(subcommand);
    }
    app
}

fn run_shell_commands(
    commands: &[String],
    flag_values: &HashMap<String, String>,
    verbose: bool,
    base_dir: Option<&str>,
) -> Result<String, RingError> {
    let mut output_text = String::new();
    for cmd in commands {
        let replaced_cmd = replace_placeholders(cmd, flag_values, verbose);
        let replaced_cmd = replace_env_vars(&replaced_cmd, verbose)?;

        let mut command = ShellCommand::new("sh");
        command.arg("-c").arg(&replaced_cmd);
        if let Some(dir) = base_dir {
            command.current_dir(dir);
        }

        let output = command.output().map_err(|e| RingError::ShellCommand {
            command: replaced_cmd.clone(),
            code: -1,
            stderr: e.to_string(),
        })?;

        if output.status.success() {
            output_text.push_str(&String::from_utf8_lossy(&output.stdout));
        } else {
            return Err(RingError::ShellCommand {
                command: replaced_cmd,
                code: output.status.code().unwrap_or(-1),
                stderr: String::from_utf8_lossy(&output.stderr).to_string(),
            });
        }
    }
    Ok(output_text)
}

pub async fn execute_http_request(
    http: &Http,
    flag_values: &HashMap<String, String>,
    verbose: bool,
) -> Result<String, RingError> {
    let client = reqwest::Client::new();

    let replace = |template: &str| -> Result<String, RingError> {
        let result = replace_placeholders(template, flag_values, verbose);
        replace_env_vars(&result, verbose)
    };

    let url = replace(&http.url)?;
    let body = if let Some(ref body_content) = http.body {
        Some(replace(body_content)?)
    } else {
        None
    };

    let request_builder = match http.method.as_str() {
        "GET" => client.get(&url),
        "POST" => client.post(&url).body(body.unwrap_or_default()),
        "PUT" => client.put(&url).body(body.unwrap_or_default()),
        "DELETE" => client.delete(&url),
        "PATCH" => client.patch(&url).body(body.unwrap_or_default()),
        "HEAD" => client.head(&url),
        _ => return Err(RingError::UnsupportedMethod(http.method.clone())),
    };

    let mut request_with_headers = request_builder;
    if let Some(header_map) = &http.headers {
        for (header_name, header_value) in header_map.iter() {
            request_with_headers = request_with_headers.header(header_name, header_value);
        }
    }

    let response = request_with_headers.send().await.map_err(|e| RingError::Http {
        method: http.method.clone(),
        url: url.clone(),
        message: e.to_string(),
    })?;

    let text = response.text().await.map_err(|e| RingError::Http {
        method: http.method.clone(),
        url: url.clone(),
        message: e.to_string(),
    })?;

    Ok(text)
}

pub fn execute_command(
    command: &RingCommand,
    cmd_matches: &clap::ArgMatches,
    verbose: bool,
    base_dir: Option<&str>,
) -> Result<(), RingError> {
    let flag_values = extract_flag_values(&command.flags, cmd_matches);

    if verbose {
        println!("Executing command with flags: {:?}", flag_values);
    }

    if let Some(actual_cmd) = &command.cmd {
        match actual_cmd {
            CmdType::Http { http } => {
                match tokio::runtime::Runtime::new()
                    .unwrap()
                    .block_on(execute_http_request(http, &flag_values, verbose))
                {
                    Ok(output) => println!("{}", output),
                    Err(e) => return Err(e),
                }
            }
            CmdType::Run { run } => {
                match run_shell_commands(run, &flag_values, verbose, base_dir) {
                    Ok(output) => {
                        if !output.trim().is_empty() {
                            println!("{}", output);
                        }
                    }
                    Err(e) => return Err(e),
                }
            }
        }
    }

    if let Some(subcommands) = &command.subcommands {
        for (sub_name, sub_cmd) in subcommands {
            if let Some(sub_cmd_matches) = cmd_matches.subcommand_matches(sub_name) {
                execute_command(sub_cmd, sub_cmd_matches, verbose, base_dir)?;
            }
        }
    }
    Ok(())
}
```

**Step 2: Commit**

```bash
git add src/cli.rs
git commit -m "refactor: rewrite CLI module for clap v4 builder API"
```

---

### Task 6: Update Main with Init Command

**Files:**
- Modify: `src/main.rs`

**Step 1: Rewrite `src/main.rs`**

Changes:
- Add `mod errors`
- Handle `init` subcommand before config loading
- Use anyhow for top-level errors
- Pass config path properly (clap v4 API)

```rust
mod cli;
mod errors;
mod models;
mod utils;

use std::fs;
use std::path::PathBuf;

fn default_config_dir() -> PathBuf {
    dirs::home_dir()
        .expect("Unable to determine home directory")
        .join(".ring-cli/configurations")
}

fn handle_init(path: Option<&String>) -> Result<(), anyhow::Error> {
    let target = if let Some(p) = path {
        PathBuf::from(p)
    } else {
        let dir = default_config_dir();
        fs::create_dir_all(&dir)?;
        dir.join("example.yml")
    };

    if target.exists() {
        anyhow::bail!("File already exists: {}", target.display());
    }

    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)?;
    }

    let template = r#"# Ring-CLI Configuration
# See https://github.com/user/ring-cli for documentation

version: "1.0"
description: "My custom CLI"
slug: "mycli"
commands:
  greet:
    description: "Greet a user"
    flags:
      - name: "name"
        short: "n"
        description: "Name of the user to greet"
    cmd:
      run:
        - "echo Hello, ${{name}}!"

  # Example HTTP command:
  # api-status:
  #   description: "Check API status"
  #   flags: []
  #   cmd:
  #     http:
  #       method: "GET"
  #       url: "https://httpbin.org/get"

  # Example with environment variables:
  # deploy:
  #   description: "Deploy with auth"
  #   flags:
  #     - name: "target"
  #       short: "t"
  #       description: "Deploy target"
  #   cmd:
  #     run:
  #       - "curl -H 'Authorization: Bearer ${{env.API_TOKEN}}' https://${{target}}/deploy"

  # Example with subcommands:
  # db:
  #   description: "Database operations"
  #   flags: []
  #   subcommands:
  #     migrate:
  #       description: "Run migrations"
  #       flags: []
  #       cmd:
  #         run:
  #           - "echo Running migrations..."
  #     seed:
  #       description: "Seed database"
  #       flags: []
  #       cmd:
  #         run:
  #           - "echo Seeding database..."
"#;

    fs::write(&target, template)?;
    println!("Created configuration at: {}", target.display());
    Ok(())
}

fn main() -> anyhow::Result<()> {
    // Check for init command early (before config loading)
    let args: Vec<String> = std::env::args().collect();
    if args.len() >= 2 && args[1] == "init" {
        let path = args.iter().position(|a| a == "--path").and_then(|i| args.get(i + 1));
        return handle_init(path).map_err(Into::into);
    }

    // Extract config path before full CLI parsing
    let config_path = args.iter()
        .find(|arg| arg.starts_with("--config="))
        .and_then(|arg| arg.split('=').nth(1).map(String::from))
        .or_else(|| {
            args.iter()
                .position(|a| a == "-c" || a == "--config")
                .and_then(|i| args.get(i + 1).cloned())
        });

    let configurations = utils::load_configurations(config_path.as_deref())?;

    let matches = cli::build_cli_from_configs(&configurations).get_matches();

    let is_quiet = matches.get_flag("quiet");
    let is_verbose = matches.get_flag("verbose");
    let base_dir = matches.get_one::<String>("base_dir").map(|s| s.as_str());

    for config in &configurations {
        if let Some(submatches) = matches.subcommand_matches(&config.slug) {
            for (cmd_name, cmd) in &config.commands {
                if let Some(cmd_matches) = submatches.subcommand_matches(cmd_name) {
                    if let Err(e) = cli::execute_command(cmd, cmd_matches, is_verbose, base_dir) {
                        if !is_quiet {
                            eprintln!("Error: {}", e);
                        }
                        std::process::exit(1);
                    }
                }
            }
        }
    }
    Ok(())
}
```

**Step 2: Verify it compiles**

Run: `eval "$(mise activate zsh)" && cargo build 2>&1`

Expected: Successful compilation with no errors.

**Step 3: Quick smoke test**

Run: `eval "$(mise activate zsh)" && cargo run -- --help 2>&1`

Expected: Help text showing ring-cli options including quiet, verbose, config, base-dir.

**Step 4: Commit**

```bash
git add src/main.rs src/errors.rs
git commit -m "feat: update main with init command and anyhow error handling"
```

---

### Task 7: Add Test Fixtures

**Files:**
- Create: `tests/fixtures/valid_config.yml`
- Create: `tests/fixtures/valid_http_config.yml`
- Create: `tests/fixtures/invalid_both.yml`
- Create: `tests/fixtures/invalid_neither.yml`

**Step 1: Create test fixtures directory and files**

`tests/fixtures/valid_config.yml`:
```yaml
version: "1.0"
description: "Test CLI"
slug: "test"
commands:
  greet:
    description: "Greet a user"
    flags:
      - name: "name"
        short: "n"
        description: "Name of the user"
    cmd:
      run:
        - "echo Hello, ${{name}}!"
  multi:
    description: "Multi-step command"
    flags: []
    cmd:
      run:
        - "echo step1"
        - "echo step2"
```

`tests/fixtures/valid_http_config.yml`:
```yaml
version: "1.0"
description: "HTTP Test CLI"
slug: "httptest"
commands:
  fetch:
    description: "Fetch a URL"
    flags:
      - name: "url"
        description: "URL to fetch"
    cmd:
      http:
        method: "GET"
        url: "${{url}}"
```

`tests/fixtures/invalid_both.yml`:
```yaml
version: "1.0"
description: "Invalid config"
slug: "invalid"
commands:
  bad:
    description: "Has both cmd and subcommands"
    flags: []
    cmd:
      run:
        - "echo hello"
    subcommands:
      sub:
        description: "A subcommand"
        flags: []
        cmd:
          run:
            - "echo sub"
```

`tests/fixtures/invalid_neither.yml`:
```yaml
version: "1.0"
description: "Invalid config"
slug: "invalid"
commands:
  bad:
    description: "Has neither cmd nor subcommands"
    flags: []
```

**Step 2: Commit**

```bash
git add tests/fixtures/
git commit -m "test: add YAML fixture files for testing"
```

---

### Task 8: Add Unit Tests for Models

**Files:**
- Modify: `src/models.rs` (append `#[cfg(test)]` module)

**Step 1: Add test module to bottom of `src/models.rs`**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deserialize_run_command() {
        let yaml = r#"
version: "1.0"
description: "Test"
slug: "test"
commands:
  greet:
    description: "Greet"
    flags:
      - name: "name"
        short: "n"
        description: "Name"
    cmd:
      run:
        - "echo Hello, ${{name}}!"
"#;
        let config: Configuration = serde_yml::from_str(yaml).unwrap();
        assert_eq!(config.slug, "test");
        assert_eq!(config.commands.len(), 1);
        let cmd = &config.commands["greet"];
        assert_eq!(cmd.flags.len(), 1);
        assert_eq!(cmd.flags[0].name, "name");
        assert_eq!(cmd.flags[0].short, Some("n".to_string()));
        assert!(matches!(cmd.cmd, Some(CmdType::Run { .. })));
    }

    #[test]
    fn test_deserialize_http_command() {
        let yaml = r#"
version: "1.0"
description: "Test"
slug: "test"
commands:
  fetch:
    description: "Fetch"
    flags: []
    cmd:
      http:
        method: "GET"
        url: "https://example.com"
        headers:
          Authorization: "Bearer token"
        body: "test body"
"#;
        let config: Configuration = serde_yml::from_str(yaml).unwrap();
        let cmd = &config.commands["fetch"];
        if let Some(CmdType::Http { http }) = &cmd.cmd {
            assert_eq!(http.method, "GET");
            assert_eq!(http.url, "https://example.com");
            assert!(http.headers.is_some());
            assert_eq!(http.body, Some("test body".to_string()));
        } else {
            panic!("Expected Http command type");
        }
    }

    #[test]
    fn test_validate_rejects_both_cmd_and_subcommands() {
        let cmd = Command {
            description: "bad".to_string(),
            flags: vec![],
            cmd: Some(CmdType::Run {
                run: vec!["echo hi".to_string()],
            }),
            subcommands: Some(HashMap::new()),
        };
        let result = cmd.validate("test");
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("not both"));
    }

    #[test]
    fn test_validate_rejects_neither_cmd_nor_subcommands() {
        let cmd = Command {
            description: "bad".to_string(),
            flags: vec![],
            cmd: None,
            subcommands: None,
        };
        let result = cmd.validate("test");
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("must be present"));
    }

    #[test]
    fn test_validate_accepts_cmd_only() {
        let cmd = Command {
            description: "ok".to_string(),
            flags: vec![],
            cmd: Some(CmdType::Run {
                run: vec!["echo hi".to_string()],
            }),
            subcommands: None,
        };
        assert!(cmd.validate("test").is_ok());
    }

    #[test]
    fn test_validate_accepts_subcommands_only() {
        let mut subs = HashMap::new();
        subs.insert(
            "sub".to_string(),
            Command {
                description: "sub".to_string(),
                flags: vec![],
                cmd: Some(CmdType::Run {
                    run: vec!["echo sub".to_string()],
                }),
                subcommands: None,
            },
        );
        let cmd = Command {
            description: "parent".to_string(),
            flags: vec![],
            cmd: None,
            subcommands: Some(subs),
        };
        assert!(cmd.validate("test").is_ok());
    }

    #[test]
    fn test_validate_error_includes_context_path() {
        let mut subs = HashMap::new();
        subs.insert(
            "broken".to_string(),
            Command {
                description: "broken".to_string(),
                flags: vec![],
                cmd: None,
                subcommands: None,
            },
        );
        let cmd = Command {
            description: "parent".to_string(),
            flags: vec![],
            cmd: None,
            subcommands: Some(subs),
        };
        let err = cmd.validate("mycli > deploy").unwrap_err().to_string();
        assert!(err.contains("mycli > deploy > broken"));
    }

    #[test]
    fn test_deserialize_flags_without_short() {
        let yaml = r#"
version: "1.0"
description: "Test"
slug: "test"
commands:
  cmd:
    description: "Test"
    flags:
      - name: "longonly"
        description: "No short form"
    cmd:
      run:
        - "echo test"
"#;
        let config: Configuration = serde_yml::from_str(yaml).unwrap();
        let cmd = &config.commands["cmd"];
        assert_eq!(cmd.flags[0].short, None);
    }
}
```

**Step 2: Run tests**

Run: `eval "$(mise activate zsh)" && cargo test -- --nocapture 2>&1`

Expected: All model tests pass.

**Step 3: Commit**

```bash
git add src/models.rs
git commit -m "test: add unit tests for models module"
```

---

### Task 9: Add Unit Tests for Utils

**Files:**
- Modify: `src/utils.rs` (append `#[cfg(test)]` module)

**Step 1: Add test module to bottom of `src/utils.rs`**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    #[test]
    fn test_replace_single_placeholder() {
        let mut flags = HashMap::new();
        flags.insert("name".to_string(), "Alice".to_string());
        let result = replace_placeholders("Hello, ${{name}}!", &flags, false);
        assert_eq!(result, "Hello, Alice!");
    }

    #[test]
    fn test_replace_multiple_placeholders() {
        let mut flags = HashMap::new();
        flags.insert("first".to_string(), "Alice".to_string());
        flags.insert("last".to_string(), "Smith".to_string());
        let result = replace_placeholders("${{first}} ${{last}}", &flags, false);
        assert_eq!(result, "Alice Smith");
    }

    #[test]
    fn test_replace_missing_placeholder_left_as_is() {
        let flags = HashMap::new();
        let result = replace_placeholders("Hello, ${{name}}!", &flags, false);
        assert_eq!(result, "Hello, ${{name}}!");
    }

    #[test]
    fn test_replace_env_var() {
        std::env::set_var("RING_TEST_VAR", "test_value");
        let result = replace_env_vars("token=${{env.RING_TEST_VAR}}", false).unwrap();
        assert_eq!(result, "token=test_value");
        std::env::remove_var("RING_TEST_VAR");
    }

    #[test]
    fn test_replace_env_var_not_set() {
        std::env::remove_var("RING_NONEXISTENT_VAR");
        let result = replace_env_vars("${{env.RING_NONEXISTENT_VAR}}", false);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("RING_NONEXISTENT_VAR"));
    }

    #[test]
    fn test_load_single_config_file() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("test.yml");
        let mut file = fs::File::create(&file_path).unwrap();
        write!(
            file,
            r#"version: "1.0"
description: "Test"
slug: "test"
commands:
  hello:
    description: "Say hello"
    flags: []
    cmd:
      run:
        - "echo hello"
"#
        )
        .unwrap();

        let configs = load_configurations(Some(file_path.to_str().unwrap())).unwrap();
        assert_eq!(configs.len(), 1);
        assert_eq!(configs[0].slug, "test");
    }

    #[test]
    fn test_load_config_directory() {
        let dir = TempDir::new().unwrap();
        for name in &["a.yml", "b.yaml"] {
            let file_path = dir.path().join(name);
            let mut file = fs::File::create(&file_path).unwrap();
            write!(
                file,
                r#"version: "1.0"
description: "Test {name}"
slug: "{slug}"
commands:
  cmd:
    description: "A command"
    flags: []
    cmd:
      run:
        - "echo test"
"#,
                name = name,
                slug = name.replace('.', "_")
            )
            .unwrap();
        }
        // Also create a non-yaml file that should be ignored
        fs::write(dir.path().join("readme.txt"), "ignore me").unwrap();

        let configs = load_configurations(Some(dir.path().to_str().unwrap())).unwrap();
        assert_eq!(configs.len(), 2);
    }

    #[test]
    fn test_load_nonexistent_path_errors() {
        let result = load_configurations(Some("/nonexistent/path/to/config.yml"));
        assert!(result.is_err());
    }

    #[test]
    fn test_load_invalid_yaml_shows_path() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("bad.yml");
        fs::write(&file_path, "not: [valid: yaml: config").unwrap();

        let result = load_configurations(Some(file_path.to_str().unwrap()));
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("bad.yml"));
    }

    #[test]
    fn test_load_validates_configs() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("invalid.yml");
        let mut file = fs::File::create(&file_path).unwrap();
        write!(
            file,
            r#"version: "1.0"
description: "Test"
slug: "test"
commands:
  bad:
    description: "Has neither"
    flags: []
"#
        )
        .unwrap();

        let result = load_configurations(Some(file_path.to_str().unwrap()));
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("must be present"));
    }
}
```

**Step 2: Add tempfile dev-dependency to Cargo.toml**

Append to `Cargo.toml`:
```toml
[dev-dependencies]
tempfile = "3"
```

**Step 3: Run tests**

Run: `eval "$(mise activate zsh)" && cargo test -- --nocapture 2>&1`

Expected: All tests pass (models + utils).

**Step 4: Commit**

```bash
git add src/utils.rs Cargo.toml
git commit -m "test: add unit tests for utils module"
```

---

### Task 10: Add Unit Tests for CLI

**Files:**
- Modify: `src/cli.rs` (append `#[cfg(test)]` module)

**Step 1: Add test module to bottom of `src/cli.rs`**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{CmdType, Command as RingCommand, Configuration, Flag};
    use std::collections::HashMap;

    fn make_test_config() -> Vec<Configuration> {
        let mut commands = HashMap::new();
        commands.insert(
            "greet".to_string(),
            RingCommand {
                description: "Greet someone".to_string(),
                flags: vec![Flag {
                    name: "name".to_string(),
                    short: Some("n".to_string()),
                    description: "Name".to_string(),
                }],
                cmd: Some(CmdType::Run {
                    run: vec!["echo Hello, ${{name}}!".to_string()],
                }),
                subcommands: None,
            },
        );
        vec![Configuration {
            version: "1.0".to_string(),
            description: "Test".to_string(),
            slug: "test".to_string(),
            commands,
        }]
    }

    #[test]
    fn test_build_cli_has_global_flags() {
        let configs = make_test_config();
        let app = build_cli_from_configs(&configs);
        let matches = app.try_get_matches_from(["ring-cli", "--help"]);
        // --help causes an error (it exits), but the point is the app builds
        assert!(matches.is_err()); // help triggers exit
    }

    #[test]
    fn test_build_cli_has_config_subcommand() {
        let configs = make_test_config();
        let app = build_cli_from_configs(&configs);
        let matches = app
            .try_get_matches_from(["ring-cli", "test", "greet", "--name", "Alice"])
            .unwrap();
        let sub = matches.subcommand_matches("test").unwrap();
        let greet = sub.subcommand_matches("greet").unwrap();
        let name = greet.get_one::<String>("name").unwrap();
        assert_eq!(name, "Alice");
    }

    #[test]
    fn test_build_cli_quiet_and_verbose_flags() {
        let configs = make_test_config();
        let app = build_cli_from_configs(&configs);
        let matches = app
            .try_get_matches_from(["ring-cli", "-q", "-v", "test", "greet", "--name", "X"])
            .unwrap();
        assert!(matches.get_flag("quiet"));
        assert!(matches.get_flag("verbose"));
    }

    #[test]
    fn test_build_cli_nested_subcommands() {
        let mut inner_cmds = HashMap::new();
        inner_cmds.insert(
            "migrate".to_string(),
            RingCommand {
                description: "Run migrations".to_string(),
                flags: vec![],
                cmd: Some(CmdType::Run {
                    run: vec!["echo migrating".to_string()],
                }),
                subcommands: None,
            },
        );

        let mut commands = HashMap::new();
        commands.insert(
            "db".to_string(),
            RingCommand {
                description: "Database ops".to_string(),
                flags: vec![],
                cmd: None,
                subcommands: Some(inner_cmds),
            },
        );

        let configs = vec![Configuration {
            version: "1.0".to_string(),
            description: "Test".to_string(),
            slug: "app".to_string(),
            commands,
        }];

        let app = build_cli_from_configs(&configs);
        let matches = app
            .try_get_matches_from(["ring-cli", "app", "db", "migrate"])
            .unwrap();
        let app_m = matches.subcommand_matches("app").unwrap();
        let db_m = app_m.subcommand_matches("db").unwrap();
        assert!(db_m.subcommand_matches("migrate").is_some());
    }

    #[test]
    fn test_extract_flag_values() {
        let configs = make_test_config();
        let app = build_cli_from_configs(&configs);
        let matches = app
            .try_get_matches_from(["ring-cli", "test", "greet", "--name", "Bob"])
            .unwrap();
        let sub = matches.subcommand_matches("test").unwrap();
        let greet = sub.subcommand_matches("greet").unwrap();

        let flags = vec![Flag {
            name: "name".to_string(),
            short: Some("n".to_string()),
            description: "Name".to_string(),
        }];

        let values = extract_flag_values(&flags, greet);
        assert_eq!(values.get("name").unwrap(), "Bob");
    }
}
```

**Step 2: Run tests**

Run: `eval "$(mise activate zsh)" && cargo test -- --nocapture 2>&1`

Expected: All tests pass.

**Step 3: Commit**

```bash
git add src/cli.rs
git commit -m "test: add unit tests for CLI module"
```

---

### Task 11: Add Integration Tests

**Files:**
- Create: `tests/integration.rs`

**Step 1: Create `tests/integration.rs`**

```rust
use std::process::Command;

fn cargo_bin() -> Command {
    let mut cmd = Command::new("cargo");
    cmd.args(["run", "--"]);
    cmd
}

#[test]
fn test_help_output() {
    let output = cargo_bin()
        .arg("--help")
        .output()
        .expect("Failed to run ring-cli");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Ring CLI Tool"));
    assert!(stdout.contains("--quiet"));
    assert!(stdout.contains("--verbose"));
    assert!(stdout.contains("--config"));
    assert!(stdout.contains("--base-dir"));
}

#[test]
fn test_version_output() {
    let output = cargo_bin()
        .arg("--version")
        .output()
        .expect("Failed to run ring-cli");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("1.1.0"));
}

#[test]
fn test_load_fixture_config_and_run_command() {
    let output = cargo_bin()
        .args(["--config=tests/fixtures/valid_config.yml", "test", "greet", "--name", "World"])
        .output()
        .expect("Failed to run ring-cli");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Hello, World!"),
        "Expected 'Hello, World!' in output, got: {}",
        stdout
    );
}

#[test]
fn test_multi_step_command() {
    let output = cargo_bin()
        .args(["--config=tests/fixtures/valid_config.yml", "test", "multi"])
        .output()
        .expect("Failed to run ring-cli");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("step1"));
    assert!(stdout.contains("step2"));
}

#[test]
fn test_invalid_config_both_cmd_and_subcommands() {
    let output = cargo_bin()
        .args(["--config=tests/fixtures/invalid_both.yml"])
        .output()
        .expect("Failed to run ring-cli");
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("not both"));
}

#[test]
fn test_invalid_config_neither_cmd_nor_subcommands() {
    let output = cargo_bin()
        .args(["--config=tests/fixtures/invalid_neither.yml"])
        .output()
        .expect("Failed to run ring-cli");
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("must be present"));
}

#[test]
fn test_nonexistent_config_path() {
    let output = cargo_bin()
        .args(["--config=/nonexistent/path.yml"])
        .output()
        .expect("Failed to run ring-cli");
    assert!(!output.status.success());
}

#[test]
fn test_init_creates_file() {
    let dir = tempfile::TempDir::new().unwrap();
    let target = dir.path().join("init-test.yml");

    let output = cargo_bin()
        .args(["init", "--path", target.to_str().unwrap()])
        .output()
        .expect("Failed to run ring-cli init");

    assert!(output.status.success());
    assert!(target.exists());

    let content = std::fs::read_to_string(&target).unwrap();
    assert!(content.contains("slug:"));
    assert!(content.contains("commands:"));
}

#[test]
fn test_init_refuses_overwrite() {
    let dir = tempfile::TempDir::new().unwrap();
    let target = dir.path().join("exists.yml");
    std::fs::write(&target, "existing content").unwrap();

    let output = cargo_bin()
        .args(["init", "--path", target.to_str().unwrap()])
        .output()
        .expect("Failed to run ring-cli init");

    assert!(!output.status.success());
}

#[test]
fn test_env_var_replacement() {
    std::env::set_var("RING_TEST_GREETING", "Howdy");

    let dir = tempfile::TempDir::new().unwrap();
    let config_path = dir.path().join("env_test.yml");
    std::fs::write(
        &config_path,
        r#"version: "1.0"
description: "Env test"
slug: "envtest"
commands:
  say:
    description: "Say something"
    flags: []
    cmd:
      run:
        - "echo ${{env.RING_TEST_GREETING}}"
"#,
    )
    .unwrap();

    let output = cargo_bin()
        .args([
            &format!("--config={}", config_path.display()),
            "envtest",
            "say",
        ])
        .output()
        .expect("Failed to run ring-cli");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Howdy"),
        "Expected 'Howdy' in output, got: {}",
        stdout
    );

    std::env::remove_var("RING_TEST_GREETING");
}
```

**Step 2: Run all tests**

Run: `eval "$(mise activate zsh)" && cargo test -- --nocapture 2>&1`

Expected: All unit and integration tests pass.

**Step 3: Commit**

```bash
git add tests/
git commit -m "test: add integration tests for CLI commands, init, and env vars"
```

---

### Task 12: Update CI/CD Workflow

**Files:**
- Modify: `.github/workflows/ci.yml`

**Step 1: Update `.github/workflows/ci.yml`**

Changes:
- `actions/checkout@v3` → `actions/checkout@v4`
- `Swatinem/rust-cache@v2` → `Swatinem/rust-cache@v2` (still latest)
- `actions/upload-artifact@v3` → `actions/upload-artifact@v4`
- `softprops/action-gh-release@v1` → `softprops/action-gh-release@v2`
- `ubuntu-20.04` → `ubuntu-latest` for all Linux targets
- Remove `body_path: Changes.md` from release step (file was removed)
- Remove the `musl-tools` install step condition check (keep the step, it still applies to musl targets)

The full file is long (197 lines). The specific edits needed:

1. Line 22, 28, 34, 40, 46, 50, 56, 60, 64, 69, 74, 79, 84, 89, 94, 98: Replace `os: ubuntu-20.04` with `os: ubuntu-latest`
2. Line 135: Replace `actions/checkout@v3` with `actions/checkout@v4`
3. Line 179: Replace `actions/upload-artifact@v3` with `actions/upload-artifact@v4`
4. Line 191: Replace `softprops/action-gh-release@v1` with `softprops/action-gh-release@v2`
5. Line 195: Remove `body_path: Changes.md`

**Step 2: Run a local build to verify nothing broke**

Run: `eval "$(mise activate zsh)" && cargo test 2>&1`

Expected: All tests still pass.

**Step 3: Commit**

```bash
git add .github/workflows/ci.yml
git commit -m "ci: update GitHub Actions versions and ubuntu runner"
```

---

### Task 13: Run Full Audit and Verify Clean

**Step 1: Regenerate Cargo.lock and run audit**

Run: `eval "$(mise activate zsh)" && cargo audit 2>&1`

Expected: 0 vulnerabilities (or dramatically reduced — some transitive deps may still have advisories).

**Step 2: Run full test suite**

Run: `eval "$(mise activate zsh)" && cargo test 2>&1`

Expected: All tests pass.

**Step 3: Run clippy for code quality**

Run: `eval "$(mise activate zsh)" && cargo clippy -- -D warnings 2>&1`

Expected: No warnings.

**Step 4: Fix any remaining issues found by clippy or audit**

Address as needed.

**Step 5: Final commit**

```bash
git add -A
git commit -m "chore: clean up any remaining clippy warnings and lock file"
```

---

## Task Summary

| Task | Description | Est. |
|------|-------------|------|
| 1 | Update Cargo.toml dependencies | 2 min |
| 2 | Create error types module | 3 min |
| 3 | Update models module | 3 min |
| 4 | Update utils module | 5 min |
| 5 | Rewrite CLI module for clap v4 | 5 min |
| 6 | Update main with init command | 5 min |
| 7 | Add test fixtures | 2 min |
| 8 | Add unit tests for models | 3 min |
| 9 | Add unit tests for utils | 3 min |
| 10 | Add unit tests for CLI | 3 min |
| 11 | Add integration tests | 5 min |
| 12 | Update CI/CD workflow | 3 min |
| 13 | Run full audit and verify clean | 5 min |
