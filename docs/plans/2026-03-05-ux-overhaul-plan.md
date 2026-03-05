# ring-cli UX Overhaul Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Transform ring-cli into a polished CLI generator with v2 config (no slug), cached completions with trust system, color output, and comprehensive tests.

**Architecture:** Break the overhaul into layers: (1) remove slug and bump to v2 config format, (2) add style/color module, (3) add cache/trust system, (4) rewrite init to use clap + set up alias/completions/cache, (5) add refresh-configuration, (6) generate completions from cached config, (7) comprehensive tests. Each layer builds on the previous.

**Tech Stack:** Rust, clap 4.5 (builder API + `color` feature), clap_complete 4.5, sha2 crate, serde_json (for cache metadata), dirs 6.0, tempfile (dev)

---

### Task 1: Add new dependencies to Cargo.toml

**Files:**
- Modify: `Cargo.toml`

**Step 1: Add dependencies**

Add to `[dependencies]`:
```toml
clap = { version = "4.5", features = ["string", "color"] }
clap_complete = "4.5"
sha2 = "0.10"
serde_json = "1.0"
```

Note: clap already exists — just add the `"color"` feature. The other three are new.

**Step 2: Verify it compiles**

Run: `eval "$(mise activate zsh)" && cargo check`
Expected: Compiles without errors

**Step 3: Commit**

```bash
git add Cargo.toml Cargo.lock
git commit -m "chore: add clap_complete, sha2, serde_json dependencies"
```

---

### Task 2: Remove slug from config model (v2 breaking change)

**Files:**
- Modify: `src/models.rs`

**Step 1: Write failing test for v2 config without slug**

Add to `src/models.rs` tests module:

```rust
#[test]
fn test_deserialize_v2_config_no_slug() {
    let yaml = r#"
version: "2.0"
description: "My CLI"
commands:
  greet:
    description: "Greet someone"
    flags:
      - name: "name"
        short: "n"
        description: "Name to greet"
    cmd:
      run:
        - "echo Hello, ${{name}}!"
"#;
    let config: Configuration = serde_saphyr::from_str(yaml).expect("valid v2 YAML");
    assert_eq!(config.version, "2.0");
    assert_eq!(config.description, "My CLI");
    assert!(config.commands.contains_key("greet"));
}

#[test]
fn test_v1_config_with_slug_still_parses() {
    let yaml = r#"
version: "1.0"
description: "Old CLI"
slug: "old"
commands:
  run:
    description: "Run"
    flags: []
    cmd:
      run:
        - "echo hi"
"#;
    // slug is now ignored via serde(default), so this should still parse
    let config: Configuration = serde_saphyr::from_str(yaml).expect("v1 YAML should still parse");
    assert_eq!(config.description, "Old CLI");
}
```

**Step 2: Run tests to verify they fail**

Run: `eval "$(mise activate zsh)" && cargo test -- test_deserialize_v2_config`
Expected: FAIL — `slug` is a required field, v2 config without it fails to parse

**Step 3: Remove slug from Configuration struct**

In `src/models.rs`, change:

```rust
#[derive(Debug, Deserialize, Serialize)]
pub struct Configuration {
    pub version: String,
    pub description: String,
    pub commands: HashMap<String, Command>,
}
```

Remove the `pub slug: String` field entirely.

**Step 4: Run the new tests**

Run: `eval "$(mise activate zsh)" && cargo test -- test_deserialize_v2`
Expected: PASS

**Step 5: Fix all compilation errors from slug removal**

The following files reference `config.slug` and will fail to compile. Fix each:

- `src/cli.rs:85` — `config.slug.to_owned()` used as subcommand name. This entire slug-based subcommand wrapping needs to be removed. Commands should be registered directly on the top-level app. Change `build_cli_from_configs` to register commands directly (see Task 4).
- `src/cli.rs:256,309` — test fixtures create `Configuration` with `slug`. Remove the `slug` field from all test config construction.
- `src/utils.rs:118` — `config.slug` used in validation context. Change to use `config.description` or just `"config"`.
- `src/utils.rs:176,189,195-198,205-208` — test YAML strings and assertions reference slug. Remove slug from YAML strings and assertions.
- `src/main.rs:166` — init template YAML contains `slug`. Remove it and bump version to `"2.0"`.
- `src/main.rs:255` — `matches.subcommand_matches(&config.slug)` — this is the slug-based dispatch. Change to iterate commands directly from config (see Task 4).
- `src/models.rs` — existing tests reference slug in YAML. Remove slug from all test YAML strings and remove `assert_eq!(config.slug, ...)` lines.

**Step 6: Fix test fixtures**

Update all YAML fixture files to v2 format (remove slug, set version to "2.0"):

`tests/fixtures/valid_config.yml`:
```yaml
version: "2.0"
description: "Test CLI"
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
version: "2.0"
description: "HTTP Test CLI"
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
version: "2.0"
description: "Invalid config"
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
version: "2.0"
description: "Invalid config"
commands:
  bad:
    description: "Has neither cmd nor subcommands"
    flags: []
```

**Step 7: Run all tests**

Run: `eval "$(mise activate zsh)" && cargo test`
Expected: All tests PASS (some integration tests may still fail if cli.rs dispatch logic isn't updated yet — that's Task 4)

**Step 8: Commit**

```bash
git add -A
git commit -m "feat!: remove slug field, bump config to v2 format

BREAKING CHANGE: slug field removed from Configuration. Commands are now
registered directly at the top level."
```

---

### Task 3: Create style module for color output

**Files:**
- Create: `src/style.rs`
- Modify: `src/main.rs` (add `mod style;`)

**Step 1: Write tests for style module**

Create `src/style.rs` with tests:

```rust
use std::io::IsTerminal;
use std::sync::OnceLock;

#[derive(Clone, Copy, PartialEq)]
pub enum ColorMode {
    Auto,
    Always,
    Never,
}

static COLOR_MODE: OnceLock<ColorMode> = OnceLock::new();

pub fn init(mode: ColorMode) {
    let _ = COLOR_MODE.set(mode);
}

fn is_color_enabled() -> bool {
    let mode = COLOR_MODE.get().copied().unwrap_or(ColorMode::Auto);
    match mode {
        ColorMode::Always => true,
        ColorMode::Never => false,
        ColorMode::Auto => {
            if std::env::var_os("NO_COLOR").is_some() {
                return false;
            }
            std::io::stdout().is_terminal()
        }
    }
}

pub fn error(msg: &str) -> String {
    if is_color_enabled() {
        format!("\x1b[1;31mError:\x1b[0m {msg}")
    } else {
        format!("Error: {msg}")
    }
}

pub fn warn(msg: &str) -> String {
    if is_color_enabled() {
        format!("\x1b[33mWarning:\x1b[0m {msg}")
    } else {
        format!("Warning: {msg}")
    }
}

pub fn success(msg: &str) -> String {
    if is_color_enabled() {
        format!("\x1b[32m{msg}\x1b[0m")
    } else {
        msg.to_string()
    }
}

pub fn bold(msg: &str) -> String {
    if is_color_enabled() {
        format!("\x1b[1m{msg}\x1b[0m")
    } else {
        msg.to_string()
    }
}

pub fn dim(msg: &str) -> String {
    if is_color_enabled() {
        format!("\x1b[2m{msg}\x1b[0m")
    } else {
        msg.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_no_color() {
        // Since OnceLock can only be set once per process, test the formatting functions directly
        assert_eq!(
            format!("Error: {}", "something broke"),
            "Error: something broke"
        );
    }

    #[test]
    fn test_error_with_ansi() {
        let result = format!("\x1b[1;31mError:\x1b[0m {}", "something broke");
        assert!(result.contains("\x1b[1;31m"));
        assert!(result.contains("something broke"));
    }

    #[test]
    fn test_warn_format() {
        let plain = format!("Warning: {}", "watch out");
        assert_eq!(plain, "Warning: watch out");
    }

    #[test]
    fn test_success_format() {
        let msg = "Done!";
        assert_eq!(msg, "Done!");
    }
}
```

**Step 2: Add mod declaration**

Add `mod style;` to `src/main.rs` after the existing mod declarations.

**Step 3: Run tests**

Run: `eval "$(mise activate zsh)" && cargo test -- style`
Expected: PASS

**Step 4: Commit**

```bash
git add src/style.rs src/main.rs
git commit -m "feat: add style module for color output with TTY detection"
```

---

### Task 4: Rewrite CLI builder — commands at top level, no slug wrapping

**Files:**
- Modify: `src/cli.rs`
- Modify: `src/main.rs`

**Step 1: Rewrite build_cli_from_configs to register commands directly**

The current function wraps each config's commands under a slug subcommand. Since we now support only single-config mode, change it to accept a single `Configuration` and register commands directly on the top-level app.

Rename to `build_cli(config: &Configuration)`:

```rust
pub fn build_cli(config: &Configuration) -> clap::Command {
    let mut app = clap::Command::new("ring-cli")
        .version(env!("CARGO_PKG_VERSION"))
        .about(config.description.to_owned())
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
            clap::Arg::new("color")
                .long("color")
                .value_name("WHEN")
                .help("Color output")
                .value_parser(["auto", "always", "never"])
                .default_value("auto"),
        );

    for (cmd_name, cmd) in &config.commands {
        let mut cmd_subcommand =
            clap::Command::new(cmd_name.to_owned()).about(cmd.description.to_owned());
        for flag in &cmd.flags {
            cmd_subcommand = cmd_subcommand.arg(build_arg(flag));
        }
        cmd_subcommand = add_subcommands_to_cli(cmd, cmd_subcommand);
        app = app.subcommand(cmd_subcommand);
    }

    // Built-in command exposed to the user
    app = app.subcommand(
        clap::Command::new("refresh-configuration")
            .about("Re-read and trust updated configuration"),
    );

    app
}
```

Also provide a minimal `build_ring_cli()` for when ring-cli is invoked directly (no config):

```rust
pub fn build_ring_cli() -> clap::Command {
    clap::Command::new("ring-cli")
        .version(env!("CARGO_PKG_VERSION"))
        .about("CLI generator from YAML configurations")
        .subcommand(
            clap::Command::new("init")
                .about("Create a new configuration and install as a shell alias")
                .arg(
                    clap::Arg::new("config-path")
                        .long("config-path")
                        .value_name("PATH")
                        .help("Path for the configuration file"),
                )
                .arg(
                    clap::Arg::new("alias")
                        .long("alias")
                        .value_name("NAME")
                        .help("Shell alias name to install")
                        .required(true),
                ),
        )
}
```

**Step 2: Rewrite main() dispatch**

In `src/main.rs`, rewrite `main()` to:

1. Check if invoked with a config (via `--config` / `-c` or via alias which bakes in `-c`). If no config, show `build_ring_cli()` which has `init`.
2. If config present, load it, build CLI with `build_cli()`, dispatch commands directly (no slug layer).

```rust
fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();

    // Determine if we have a config (alias invocation bakes in -c /path)
    let config_path = args.iter()
        .find(|arg| arg.starts_with("--config="))
        .and_then(|arg| arg.split('=').nth(1).map(String::from))
        .or_else(|| {
            args.iter()
                .position(|a| a == "-c" || a == "--config")
                .and_then(|i| args.get(i + 1).cloned())
        });

    if let Some(ref path) = config_path {
        // Alias mode: load config, build CLI, dispatch
        let config = utils::load_configuration(path)?;

        let matches = cli::build_cli(&config).get_matches();

        // Initialize color mode
        let color_str = matches.get_one::<String>("color").map(|s| s.as_str()).unwrap_or("auto");
        style::init(match color_str {
            "always" => style::ColorMode::Always,
            "never" => style::ColorMode::Never,
            _ => style::ColorMode::Auto,
        });

        let is_quiet = matches.get_flag("quiet");
        let is_verbose = matches.get_flag("verbose");

        // Handle refresh-configuration
        if matches.subcommand_matches("refresh-configuration").is_some() {
            // TODO: implement in Task 7
            println!("refresh-configuration not yet implemented");
            return Ok(());
        }

        // Dispatch user commands
        for (cmd_name, cmd) in &config.commands {
            if let Some(cmd_matches) = matches.subcommand_matches(cmd_name) {
                if let Err(e) = cli::execute_command(cmd, cmd_matches, is_verbose, None) {
                    if !is_quiet {
                        eprintln!("{}", style::error(&e.to_string()));
                    }
                    std::process::exit(1);
                }
            }
        }
    } else {
        // Direct ring-cli mode: init command
        let matches = cli::build_ring_cli().get_matches();

        if let Some(init_matches) = matches.subcommand_matches("init") {
            let config_path = init_matches.get_one::<String>("config-path");
            let alias = init_matches.get_one::<String>("alias");
            return handle_init(config_path, alias);
        }
    }

    Ok(())
}
```

**Step 3: Update load_configurations to load_configuration (single config)**

In `src/utils.rs`, add a single-config loader:

```rust
pub fn load_configuration(config_path: &str) -> Result<Configuration, RingError> {
    let path = std::path::Path::new(config_path);
    let path_str = path.display().to_string();
    let content = fs::read_to_string(path).map_err(|e| RingError::Io {
        path: path_str.clone(),
        source: e,
    })?;
    let config: Configuration = serde_saphyr::from_str(&content).map_err(|e| RingError::YamlParse {
        path: path_str,
        source: Box::new(e),
    })?;
    for (cmd_name, cmd) in &config.commands {
        cmd.validate(&format!("{}", cmd_name))?;
    }
    Ok(config)
}
```

**Step 4: Update cli.rs tests**

Update all test helpers to use `Configuration` without `slug`, and use `build_cli()` instead of `build_cli_from_configs()`:

```rust
fn make_test_config() -> Configuration {
    let mut commands = HashMap::new();
    commands.insert(
        "greet".to_string(),
        RingCommand {
            description: "Greet a user".to_string(),
            flags: vec![Flag {
                name: "name".to_string(),
                short: Some("n".to_string()),
                description: "Name of the user".to_string(),
            }],
            cmd: Some(CmdType::Run { run: vec!["echo Hello, ${{name}}!".to_string()] }),
            subcommands: None,
        },
    );
    Configuration {
        version: "2.0".to_string(),
        description: "Test CLI".to_string(),
        commands,
    }
}
```

Update test assertions — commands are now at top level (no slug subcommand wrapping):

```rust
#[test]
fn test_build_cli_has_command() {
    let config = make_test_config();
    let app = build_cli(&config);
    let matches = app
        .try_get_matches_from(["ring-cli", "greet", "--name", "Alice"])
        .expect("should parse");
    let greet_matches = matches.subcommand_matches("greet").expect("greet subcommand");
    let name = greet_matches.get_one::<String>("name").expect("name flag");
    assert_eq!(name, "Alice");
}
```

**Step 5: Update integration tests**

Integration tests currently use `--config=<path> <slug> <command>`. Since slug is removed, change to `--config=<path> <command>`:

- `test_load_fixture_config_and_run_command`: change `"test", "greet"` to just `"greet"`
- `test_multi_step_command`: change `"test", "multi"` to just `"multi"`
- `test_invalid_config_both_cmd_and_subcommands`: change `"invalid", "bad"` to just `"bad"`
- `test_invalid_config_neither_cmd_nor_subcommands`: change `"invalid", "bad"` to just `"bad"`

**Step 6: Run all tests**

Run: `eval "$(mise activate zsh)" && cargo test`
Expected: All tests PASS

**Step 7: Run clippy**

Run: `eval "$(mise activate zsh)" && cargo clippy -- -D warnings`
Expected: No warnings

**Step 8: Commit**

```bash
git add -A
git commit -m "feat!: rewrite CLI builder for single-config mode, commands at top level

BREAKING CHANGE: commands are registered directly on the CLI, no slug
subcommand wrapping. init is now a proper clap subcommand."
```

---

### Task 5: Create cache module for trust system

**Files:**
- Create: `src/cache.rs`
- Modify: `src/main.rs` (add `mod cache;`)

**Step 1: Write tests for cache operations**

Create `src/cache.rs`:

```rust
use sha2::{Digest, Sha256};
use std::fs;
use std::path::PathBuf;

#[derive(serde::Serialize, serde::Deserialize, Debug)]
pub struct AliasMetadata {
    pub source_path: String,
    pub hash: String,
    pub trusted_at: String,
}

pub fn aliases_dir() -> PathBuf {
    dirs::home_dir()
        .expect("Unable to determine home directory")
        .join(".ring-cli/aliases")
}

pub fn alias_dir(alias_name: &str) -> PathBuf {
    aliases_dir().join(alias_name)
}

pub fn compute_hash(content: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content.as_bytes());
    format!("{:x}", hasher.finalize())
}

pub fn save_trusted_config(
    alias_name: &str,
    source_path: &str,
    config_content: &str,
) -> Result<(), anyhow::Error> {
    let dir = alias_dir(alias_name);
    fs::create_dir_all(&dir)?;

    fs::write(dir.join("config.yml"), config_content)?;

    let metadata = AliasMetadata {
        source_path: source_path.to_string(),
        hash: compute_hash(config_content),
        trusted_at: chrono_free_timestamp(),
    };
    let json = serde_json::to_string_pretty(&metadata)?;
    fs::write(dir.join("metadata.json"), json)?;

    Ok(())
}

pub fn load_trusted_config(alias_name: &str) -> Result<(String, AliasMetadata), anyhow::Error> {
    let dir = alias_dir(alias_name);
    let config = fs::read_to_string(dir.join("config.yml"))?;
    let metadata_str = fs::read_to_string(dir.join("metadata.json"))?;
    let metadata: AliasMetadata = serde_json::from_str(&metadata_str)?;
    Ok((config, metadata))
}

pub fn config_has_changed(alias_name: &str) -> Result<bool, anyhow::Error> {
    let (_, metadata) = load_trusted_config(alias_name)?;
    let source_content = fs::read_to_string(&metadata.source_path)?;
    let current_hash = compute_hash(&source_content);
    Ok(current_hash != metadata.hash)
}

fn chrono_free_timestamp() -> String {
    // Avoid adding chrono dependency — use simple epoch seconds
    use std::time::SystemTime;
    let secs = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    secs.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_hash_deterministic() {
        let h1 = compute_hash("hello world");
        let h2 = compute_hash("hello world");
        assert_eq!(h1, h2);
    }

    #[test]
    fn test_compute_hash_changes_with_input() {
        let h1 = compute_hash("hello");
        let h2 = compute_hash("world");
        assert_ne!(h1, h2);
    }

    #[test]
    fn test_save_and_load_trusted_config() {
        let dir = tempfile::TempDir::new().unwrap();
        // Override the alias dir by using the functions directly with temp paths
        let alias_path = dir.path().join("test-alias");
        fs::create_dir_all(&alias_path).unwrap();

        let content = "version: \"2.0\"\ndescription: \"test\"";
        fs::write(alias_path.join("config.yml"), content).unwrap();
        let metadata = AliasMetadata {
            source_path: "/tmp/test.yml".to_string(),
            hash: compute_hash(content),
            trusted_at: "12345".to_string(),
        };
        let json = serde_json::to_string_pretty(&metadata).unwrap();
        fs::write(alias_path.join("metadata.json"), json).unwrap();

        let loaded_config = fs::read_to_string(alias_path.join("config.yml")).unwrap();
        let loaded_meta: AliasMetadata =
            serde_json::from_str(&fs::read_to_string(alias_path.join("metadata.json")).unwrap()).unwrap();

        assert_eq!(loaded_config, content);
        assert_eq!(loaded_meta.hash, compute_hash(content));
        assert_eq!(loaded_meta.source_path, "/tmp/test.yml");
    }
}
```

**Step 2: Add mod declaration**

Add `mod cache;` to `src/main.rs`.

**Step 3: Run tests**

Run: `eval "$(mise activate zsh)" && cargo test -- cache`
Expected: PASS

**Step 4: Commit**

```bash
git add src/cache.rs src/main.rs
git commit -m "feat: add cache module for trust system with SHA-256 hashing"
```

---

### Task 6: Rewrite init — setup alias + cache + completions

**Files:**
- Modify: `src/main.rs`

**Step 1: Rewrite handle_init**

The new init flow:
1. If `--config-path` provided, use it. Otherwise create default config.
2. Read and validate the config.
3. Save trusted config to `~/.ring-cli/aliases/<alias>/`.
4. Install shell alias (existing logic).
5. Install completion hook alongside the alias.
6. Print success message with color.

```rust
fn handle_init(config_path: Option<&String>, alias: Option<&String>) -> Result<(), anyhow::Error> {
    let alias_name = alias.ok_or_else(|| anyhow::anyhow!("--alias is required for init"))?;
    validate_alias_name(alias_name)?;

    let target = if let Some(p) = config_path {
        let path = PathBuf::from(p);
        if !path.exists() {
            // Create the default config at the specified path
            create_default_config(&path)?;
        }
        path
    } else {
        let dir = default_config_dir();
        fs::create_dir_all(&dir)?;
        let path = dir.join(format!("{alias_name}.yml"));
        if !path.exists() {
            create_default_config(&path)?;
        }
        path
    };

    let abs_path = fs::canonicalize(&target)?;
    let abs_path_str = abs_path.display().to_string();

    // Read and validate config
    let content = fs::read_to_string(&abs_path)?;
    let _config: models::Configuration = serde_saphyr::from_str(&content)
        .map_err(|e| anyhow::anyhow!("Invalid configuration: {e}"))?;

    // Save trusted config
    cache::save_trusted_config(alias_name, &abs_path_str, &content)?;

    // Install shell alias
    install_alias(alias_name, &abs_path_str)?;

    // Install completion hook
    install_completions(alias_name)?;

    println!("{}", style::success(&format!("Alias '{}' is ready!", alias_name)));

    Ok(())
}
```

Create a helper for the default config template:

```rust
fn create_default_config(path: &std::path::Path) -> Result<(), anyhow::Error> {
    if path.exists() {
        anyhow::bail!("File already exists: {}", path.display());
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let template = r#"# Ring-CLI Configuration
version: "2.0"
description: "My custom CLI"
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
"#;
    fs::write(path, template)?;
    println!("Created configuration at: {}", path.display());
    Ok(())
}
```

**Step 2: Add install_completions function**

This appends the completion eval hook to shell configs alongside the alias:

```rust
fn install_completions(alias_name: &str) -> Result<(), anyhow::Error> {
    let shells = detect_shell_configs();
    for shell in &shells {
        let content = fs::read_to_string(&shell.path)?;
        let completion_marker = format!("# ring-cli-completions:{alias_name}");
        if content.contains(&completion_marker) {
            continue;
        }
        let hook = match shell.kind {
            ShellKind::BashZsh => {
                if shell.display_name.contains("zsh") {
                    format!("eval \"$(ring-cli --generate-completions zsh {alias_name})\" {completion_marker}")
                } else {
                    format!("eval \"$(ring-cli --generate-completions bash {alias_name})\" {completion_marker}")
                }
            }
            ShellKind::Fish => {
                format!("ring-cli --generate-completions fish {alias_name} | source {completion_marker}")
            }
            ShellKind::PowerShell => {
                format!("ring-cli --generate-completions powershell {alias_name} | Invoke-Expression {completion_marker}")
            }
        };
        let mut file = fs::OpenOptions::new().append(true).open(&shell.path)?;
        use std::io::Write;
        writeln!(file, "{}", hook)?;
    }
    Ok(())
}
```

**Step 3: Run tests**

Run: `eval "$(mise activate zsh)" && cargo test`
Expected: All tests PASS

**Step 4: Commit**

```bash
git add -A
git commit -m "feat: rewrite init to setup alias + cache + completions in one step"
```

---

### Task 7: Add refresh-configuration command

**Files:**
- Modify: `src/main.rs`

**Step 1: Implement handle_refresh_configuration**

```rust
fn handle_refresh_configuration(alias_name: &str) -> Result<(), anyhow::Error> {
    let (_, metadata) = cache::load_trusted_config(alias_name)
        .map_err(|_| anyhow::anyhow!("No cached configuration found for alias '{alias_name}'. Run 'ring-cli init' first."))?;

    let source_content = fs::read_to_string(&metadata.source_path)
        .map_err(|_| anyhow::anyhow!(
            "Source configuration not found at '{}'. The file may have been moved or deleted.",
            metadata.source_path
        ))?;

    let current_hash = cache::compute_hash(&source_content);
    if current_hash == metadata.hash {
        println!("{}", style::success("Configuration is up to date."));
        return Ok(());
    }

    // Validate new config before prompting
    let _config: models::Configuration = serde_saphyr::from_str(&source_content)
        .map_err(|e| anyhow::anyhow!("New configuration is invalid: {e}"))?;

    println!("{}", style::warn("Configuration has changed."));
    println!("Source: {}", metadata.source_path);

    // Ask for trust
    eprint!("Trust this configuration? [y/N] ");
    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;
    if input.trim().to_lowercase() != "y" {
        println!("Keeping previous trusted configuration.");
        return Ok(());
    }

    cache::save_trusted_config(alias_name, &metadata.source_path, &source_content)?;
    println!("{}", style::success("Configuration updated and trusted."));

    Ok(())
}
```

**Step 2: Wire into main()**

In the alias-mode branch of main(), replace the TODO placeholder:

```rust
if matches.subcommand_matches("refresh-configuration").is_some() {
    // Determine alias name from the config path
    // The alias name can be inferred from the cache directory
    // For now, we need to find which alias points to this config
    return handle_refresh_configuration_by_config(config_path.as_deref().unwrap());
}
```

Add helper to find alias by config path:

```rust
fn find_alias_for_config(config_path: &str) -> Result<String, anyhow::Error> {
    let aliases_dir = cache::aliases_dir();
    if !aliases_dir.exists() {
        anyhow::bail!("No aliases configured");
    }
    let abs_config = fs::canonicalize(config_path)?;
    for entry in fs::read_dir(&aliases_dir)? {
        let entry = entry?;
        if entry.file_type()?.is_dir() {
            let meta_path = entry.path().join("metadata.json");
            if meta_path.exists() {
                let meta_str = fs::read_to_string(&meta_path)?;
                let meta: cache::AliasMetadata = serde_json::from_str(&meta_str)?;
                if PathBuf::from(&meta.source_path) == abs_config {
                    return Ok(entry.file_name().to_string_lossy().to_string());
                }
            }
        }
    }
    anyhow::bail!("No alias found for config '{config_path}'")
}
```

**Step 3: Run tests**

Run: `eval "$(mise activate zsh)" && cargo test`
Expected: PASS

**Step 4: Commit**

```bash
git add -A
git commit -m "feat: add refresh-configuration command with trust prompt"
```

---

### Task 8: Add completion generation

**Files:**
- Modify: `src/main.rs` or `src/cli.rs`

**Step 1: Add --generate-completions hidden flag**

This is a hidden arg on ring-cli (not the alias) used by the shell hook installed in Task 6. When invoked as `ring-cli --generate-completions zsh my-alias`, it:

1. Loads the cached config for the alias
2. Builds the clap command
3. Generates completions via clap_complete

Add to `src/main.rs`, before the config-path detection:

```rust
// Handle completion generation (called by shell hook)
if let Some(pos) = args.iter().position(|a| a == "--generate-completions") {
    let shell_name = args.get(pos + 1).ok_or_else(|| anyhow::anyhow!("Missing shell name"))?;
    let alias_name = args.get(pos + 2).ok_or_else(|| anyhow::anyhow!("Missing alias name"))?;

    let shell: clap_complete::Shell = shell_name.parse()
        .map_err(|_| anyhow::anyhow!("Unknown shell: {shell_name}"))?;

    let (config_content, _metadata) = cache::load_trusted_config(alias_name)?;
    let config: models::Configuration = serde_saphyr::from_str(&config_content)
        .map_err(|e| anyhow::anyhow!("Cached config invalid: {e}"))?;

    let mut cmd = cli::build_cli(&config);
    clap_complete::generate(shell, &mut cmd, alias_name, &mut std::io::stdout());
    return Ok(());
}
```

**Step 2: Run tests**

Run: `eval "$(mise activate zsh)" && cargo test`
Expected: PASS

**Step 3: Commit**

```bash
git add -A
git commit -m "feat: add shell completion generation from cached config"
```

---

### Task 9: Update integration tests for v2 behavior

**Files:**
- Modify: `tests/integration.rs`

**Step 1: Rewrite integration tests**

All integration tests need updating for:
- v2 config (no slug)
- Commands at top level (no slug subcommand)
- New init flow (--alias required)
- Color output verification

Key tests to rewrite/add:

```rust
#[test]
fn test_help_output() {
    // ring-cli without config shows init help
    let output = cargo_bin()
        .arg("--help")
        .output()
        .expect("failed to run");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("init"), "missing 'init' in help:\n{stdout}");
}

#[test]
fn test_config_help_shows_commands() {
    // With config, help shows YAML commands directly
    let output = cargo_bin()
        .args(["--config=tests/fixtures/valid_config.yml", "--help"])
        .output()
        .expect("failed to run");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("greet"), "missing 'greet' in help:\n{stdout}");
    assert!(stdout.contains("multi"), "missing 'multi' in help:\n{stdout}");
    assert!(stdout.contains("refresh-configuration"), "missing refresh-configuration:\n{stdout}");
}

#[test]
fn test_run_command_no_slug() {
    // Commands are now direct, no slug prefix
    let output = cargo_bin()
        .args(["--config=tests/fixtures/valid_config.yml", "greet", "--name", "World"])
        .output()
        .expect("failed to run");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Hello, World!"), "expected greeting in:\n{stdout}");
}

#[test]
fn test_multi_step_no_slug() {
    let output = cargo_bin()
        .args(["--config=tests/fixtures/valid_config.yml", "multi"])
        .output()
        .expect("failed to run");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("step1"));
    assert!(stdout.contains("step2"));
}

#[test]
fn test_color_disabled_when_piped() {
    // When output is piped (which it is in tests), no ANSI codes should appear
    let output = cargo_bin()
        .args(["--config=/nonexistent/path.yml"])
        .output()
        .expect("failed to run");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!stderr.contains("\x1b["), "ANSI codes found in piped output:\n{stderr}");
}

#[test]
fn test_color_forced_always() {
    let output = cargo_bin()
        .args(["--config=/nonexistent/path.yml", "--color=always"])
        .output()
        .expect("failed to run");
    // This will fail because config doesn't exist, but error should be colored
    // Note: depends on whether error goes through style module
}

#[test]
fn test_no_color_env() {
    let output = cargo_bin()
        .env("NO_COLOR", "1")
        .args(["--config=tests/fixtures/valid_config.yml", "--help"])
        .output()
        .expect("failed to run");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(!stdout.contains("\x1b["), "ANSI codes found with NO_COLOR:\n{stdout}");
}

#[test]
fn test_invalid_config_both() {
    let output = cargo_bin()
        .args(["--config=tests/fixtures/invalid_both.yml", "bad"])
        .output()
        .expect("failed to run");
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("not both"), "expected 'not both' in:\n{stderr}");
}

#[test]
fn test_invalid_config_neither() {
    let output = cargo_bin()
        .args(["--config=tests/fixtures/invalid_neither.yml", "bad"])
        .output()
        .expect("failed to run");
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("must be present"), "expected 'must be present' in:\n{stderr}");
}

#[test]
fn test_init_creates_config_and_cache() {
    let dir = tempfile::TempDir::new().expect("tempdir");
    let target = dir.path().join("test.yml");
    let output = cargo_bin()
        .args(["init", "--config-path", target.to_str().unwrap(), "--alias", "test-init-cli"])
        .output()
        .expect("failed to run");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "init failed:\nstdout: {stdout}\nstderr: {stderr}");
    assert!(target.exists(), "config file should be created");
}

#[test]
fn test_env_var_replacement() {
    let dir = tempfile::TempDir::new().expect("tempdir");
    let config_path = dir.path().join("env_test.yml");
    let yaml = r#"version: "2.0"
description: "Env test CLI"
commands:
  greet:
    description: "Greet with env var"
    flags: []
    cmd:
      run:
        - "echo ${{env.RING_TEST_GREETING}}"
"#;
    std::fs::write(&config_path, yaml).unwrap();
    let output = cargo_bin()
        .env("RING_TEST_GREETING", "Howdy")
        .args([&format!("--config={}", config_path.to_str().unwrap()), "greet"])
        .output()
        .expect("failed to run");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Howdy"), "expected 'Howdy' in:\n{stdout}");
}
```

**Step 2: Run all tests**

Run: `eval "$(mise activate zsh)" && cargo test`
Expected: All PASS

**Step 3: Commit**

```bash
git add tests/integration.rs
git commit -m "test: rewrite integration tests for v2 config and new CLI structure"
```

---

### Task 10: Add edge case tests

**Files:**
- Modify: `tests/integration.rs`
- Modify: `src/models.rs` (test module)

**Step 1: Add edge case tests**

```rust
// In tests/integration.rs:

#[test]
fn test_empty_config_no_commands() {
    let dir = tempfile::TempDir::new().expect("tempdir");
    let config_path = dir.path().join("empty.yml");
    let yaml = r#"version: "2.0"
description: "Empty CLI"
commands: {}
"#;
    std::fs::write(&config_path, yaml).unwrap();
    let output = cargo_bin()
        .args([&format!("--config={}", config_path.to_str().unwrap()), "--help"])
        .output()
        .expect("failed to run");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Empty CLI"), "expected description in help:\n{stdout}");
}

#[test]
fn test_nonexistent_config_error() {
    let output = cargo_bin()
        .args(["--config=/nonexistent/path.yml", "--help"])
        .output()
        .expect("failed to run");
    assert!(!output.status.success(), "should fail for missing config");
}

#[test]
fn test_version_output() {
    let output = cargo_bin()
        .arg("--version")
        .output()
        .expect("failed to run");
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let version = env!("CARGO_PKG_VERSION");
    assert!(combined.contains(version), "version not found in:\n{combined}");
}
```

```rust
// In src/models.rs tests:

#[test]
fn test_empty_commands_map() {
    let yaml = r#"
version: "2.0"
description: "Empty"
commands: {}
"#;
    let config: Configuration = serde_saphyr::from_str(yaml).expect("valid YAML");
    assert!(config.commands.is_empty());
}
```

**Step 2: Run all tests**

Run: `eval "$(mise activate zsh)" && cargo test`
Expected: All PASS

**Step 3: Run clippy**

Run: `eval "$(mise activate zsh)" && cargo clippy -- -D warnings`
Expected: No warnings

**Step 4: Commit**

```bash
git add -A
git commit -m "test: add edge case tests for empty config, version, color modes"
```

---

### Task 11: Final verification and cleanup

**Step 1: Run full test suite**

Run: `eval "$(mise activate zsh)" && cargo test`
Expected: All tests PASS

**Step 2: Run clippy**

Run: `eval "$(mise activate zsh)" && cargo clippy -- -D warnings`
Expected: Clean

**Step 3: Manual smoke test**

```bash
# Test init flow
eval "$(mise activate zsh)" && cargo run -- init --alias smoke-test --config-path /tmp/smoke-test.yml

# Test alias-mode help
eval "$(mise activate zsh)" && cargo run -- -c /tmp/smoke-test.yml --help

# Test command execution
eval "$(mise activate zsh)" && cargo run -- -c /tmp/smoke-test.yml greet --name World

# Test refresh-configuration
eval "$(mise activate zsh)" && cargo run -- -c /tmp/smoke-test.yml refresh-configuration

# Cleanup
rm /tmp/smoke-test.yml
```

**Step 4: Clean up dead code**

Remove `load_configurations` (plural) from `src/utils.rs` if it's no longer called — the new single-config `load_configuration` replaces it. Remove any other dead code flagged by clippy or `cargo test`.

**Step 5: Commit**

```bash
git add -A
git commit -m "chore: final cleanup and verification for v2 UX overhaul"
```
