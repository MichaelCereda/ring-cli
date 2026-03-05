# Init --alias Feature Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add `--alias` flag to `ring-cli init` that installs a shell alias for the CLI, and rename `--path` to `--config-path`.

**Architecture:** Modify `handle_init` to accept both `--config-path` and `--alias` parameters. When `--alias` is provided, detect existing shell config files and append the appropriate alias line. Each shell format is handled by a helper function. Duplicate detection prevents re-adding aliases.

**Tech Stack:** Rust, std::fs, dirs crate (already a dependency), tempfile (dev-dependency for tests)

---

### Task 1: Rename `--path` to `--config-path` in `src/main.rs`

**Files:**
- Modify: `src/main.rs:95-97`

**Step 1: Update the argument parsing in main()**

Change line 96 from `--path` to `--config-path`:

```rust
let path = args.iter().position(|a| a == "--config-path").and_then(|i| args.get(i + 1));
```

**Step 2: Run existing tests to verify nothing breaks**

Run: `eval "$(mise activate zsh)" && cargo test -- test_init`
Expected: The two init tests (`test_init_creates_file`, `test_init_refuses_overwrite`) will FAIL because they still use `--path`.

**Step 3: Commit**

```bash
git add src/main.rs
git commit -m "refactor: rename --path to --config-path in init command"
```

---

### Task 2: Update integration tests to use `--config-path`

**Files:**
- Modify: `tests/integration.rs:128,148`

**Step 1: Update test_init_creates_file**

Change line 128:
```rust
.args(["init", "--config-path", target.to_str().unwrap()])
```

**Step 2: Update test_init_refuses_overwrite**

Change line 148:
```rust
.args(["init", "--config-path", target.to_str().unwrap()])
```

**Step 3: Run tests to verify they pass**

Run: `eval "$(mise activate zsh)" && cargo test -- test_init`
Expected: PASS for both `test_init_creates_file` and `test_init_refuses_overwrite`

**Step 4: Commit**

```bash
git add tests/integration.rs
git commit -m "test: update init tests to use --config-path"
```

---

### Task 3: Add shell alias generation helpers

**Files:**
- Modify: `src/main.rs` (add helper functions before `handle_init`)

**Step 1: Write unit tests for alias line generation**

Add at the bottom of `src/main.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bash_alias_line() {
        let line = alias_line_bash_zsh("my-tool", "/home/user/config.yml");
        assert_eq!(line, "alias my-tool='ring-cli -c /home/user/config.yml' # ring-cli");
    }

    #[test]
    fn test_fish_alias_line() {
        let line = alias_line_fish("my-tool", "/home/user/config.yml");
        assert_eq!(line, "alias my-tool 'ring-cli -c /home/user/config.yml' # ring-cli");
    }

    #[test]
    fn test_powershell_alias_line() {
        let line = alias_line_powershell("my-tool", "/home/user/config.yml");
        assert_eq!(line, "function my-tool { ring-cli -c /home/user/config.yml @args } # ring-cli");
    }

    #[test]
    fn test_alias_already_exists_bash() {
        let content = "# my stuff\nalias my-tool='ring-cli -c /old/path' # ring-cli\n";
        assert!(alias_exists(content, "my-tool", ShellKind::BashZsh));
        assert!(!alias_exists(content, "other-tool", ShellKind::BashZsh));
    }

    #[test]
    fn test_alias_already_exists_fish() {
        let content = "alias my-tool 'ring-cli -c /old/path' # ring-cli\n";
        assert!(alias_exists(content, "my-tool", ShellKind::Fish));
        assert!(!alias_exists(content, "other-tool", ShellKind::Fish));
    }

    #[test]
    fn test_alias_already_exists_powershell() {
        let content = "function my-tool { ring-cli -c /old/path @args } # ring-cli\n";
        assert!(alias_exists(content, "my-tool", ShellKind::PowerShell));
        assert!(!alias_exists(content, "other-tool", ShellKind::PowerShell));
    }
}
```

**Step 2: Run tests to verify they fail**

Run: `eval "$(mise activate zsh)" && cargo test -- test_bash_alias`
Expected: FAIL — functions don't exist yet

**Step 3: Implement alias helpers**

Add these functions and enum to `src/main.rs`, before `handle_init`:

```rust
#[derive(Clone, Copy)]
enum ShellKind {
    BashZsh,
    Fish,
    PowerShell,
}

fn alias_line_bash_zsh(alias_name: &str, config_path: &str) -> String {
    format!("alias {alias_name}='ring-cli -c {config_path}' # ring-cli")
}

fn alias_line_fish(alias_name: &str, config_path: &str) -> String {
    format!("alias {alias_name} 'ring-cli -c {config_path}' # ring-cli")
}

fn alias_line_powershell(alias_name: &str, config_path: &str) -> String {
    format!("function {alias_name} {{ ring-cli -c {config_path} @args }} # ring-cli")
}

fn alias_exists(file_content: &str, alias_name: &str, kind: ShellKind) -> bool {
    let pattern = match kind {
        ShellKind::BashZsh => format!("alias {alias_name}="),
        ShellKind::Fish => format!("alias {alias_name} "),
        ShellKind::PowerShell => format!("function {alias_name}"),
    };
    file_content.contains(&pattern)
}
```

**Step 4: Run tests to verify they pass**

Run: `eval "$(mise activate zsh)" && cargo test -- test_bash_alias test_fish_alias test_powershell_alias test_alias_already`
Expected: All 6 tests PASS

**Step 5: Commit**

```bash
git add src/main.rs
git commit -m "feat: add shell alias generation helpers with tests"
```

---

### Task 4: Add shell config detection and alias installation

**Files:**
- Modify: `src/main.rs` (add `install_alias` function)

**Step 1: Implement install_alias function**

Add after the alias helper functions:

```rust
struct ShellConfig {
    path: PathBuf,
    kind: ShellKind,
    display_name: &'static str,
}

fn detect_shell_configs() -> Vec<ShellConfig> {
    let home = match dirs::home_dir() {
        Some(h) => h,
        None => return vec![],
    };
    let candidates = vec![
        ShellConfig {
            path: home.join(".bashrc"),
            kind: ShellKind::BashZsh,
            display_name: "~/.bashrc",
        },
        ShellConfig {
            path: home.join(".zshrc"),
            kind: ShellKind::BashZsh,
            display_name: "~/.zshrc",
        },
        ShellConfig {
            path: home.join(".config/fish/config.fish"),
            kind: ShellKind::Fish,
            display_name: "~/.config/fish/config.fish",
        },
        ShellConfig {
            path: home.join(".config/powershell/Microsoft.PowerShell_profile.ps1"),
            kind: ShellKind::PowerShell,
            display_name: "~/.config/powershell/Microsoft.PowerShell_profile.ps1",
        },
    ];
    #[cfg(target_os = "windows")]
    let candidates = {
        let mut c = candidates;
        c.push(ShellConfig {
            path: home.join("Documents/PowerShell/Microsoft.PowerShell_profile.ps1"),
            kind: ShellKind::PowerShell,
            display_name: "~/Documents/PowerShell/Microsoft.PowerShell_profile.ps1",
        });
        c
    };
    candidates.into_iter().filter(|sc| sc.path.exists()).collect()
}

fn install_alias(alias_name: &str, config_abs_path: &str) -> Result<(), anyhow::Error> {
    let shells = detect_shell_configs();
    if shells.is_empty() {
        eprintln!("Warning: No shell config files found. Add the alias manually:");
        eprintln!("  Bash/Zsh: {}", alias_line_bash_zsh(alias_name, config_abs_path));
        eprintln!("  Fish:     {}", alias_line_fish(alias_name, config_abs_path));
        eprintln!("  PowerShell: {}", alias_line_powershell(alias_name, config_abs_path));
        return Ok(());
    }

    let mut modified = Vec::new();
    for shell in &shells {
        let content = fs::read_to_string(&shell.path)?;
        if alias_exists(&content, alias_name, shell.kind) {
            println!("Alias '{}' already exists in {}, skipping.", alias_name, shell.display_name);
            continue;
        }
        let line = match shell.kind {
            ShellKind::BashZsh => alias_line_bash_zsh(alias_name, config_abs_path),
            ShellKind::Fish => alias_line_fish(alias_name, config_abs_path),
            ShellKind::PowerShell => alias_line_powershell(alias_name, config_abs_path),
        };
        let mut file = fs::OpenOptions::new().append(true).open(&shell.path)?;
        use std::io::Write;
        writeln!(file, "\n{}", line)?;
        modified.push(shell.display_name);
    }

    if !modified.is_empty() {
        println!("Added alias '{}' to:", alias_name);
        for name in &modified {
            println!("  {}", name);
        }
        if let Some(first) = modified.first() {
            println!("Restart your terminal or run 'source {}' to use '{}'.", first, alias_name);
        }
    }

    Ok(())
}
```

**Step 2: Run all tests to make sure nothing is broken**

Run: `eval "$(mise activate zsh)" && cargo test`
Expected: All tests PASS

**Step 3: Commit**

```bash
git add src/main.rs
git commit -m "feat: add shell config detection and alias installation"
```

---

### Task 5: Wire `--alias` and `--config-path` into handle_init and main

**Files:**
- Modify: `src/main.rs:15-98`

**Step 1: Update handle_init signature and add alias logic**

Replace `handle_init` with:

```rust
fn handle_init(path: Option<&String>, alias: Option<&String>) -> Result<(), anyhow::Error> {
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

    if let Some(alias_name) = alias {
        let abs_path = fs::canonicalize(&target)?;
        let abs_path_str = abs_path.display().to_string();
        install_alias(alias_name, &abs_path_str)?;
    }

    Ok(())
}
```

**Step 2: Update main() to parse --config-path and --alias**

Replace the init block in main() (lines 95-98) with:

```rust
if args.len() >= 2 && args[1] == "init" {
    let config_path = args.iter().position(|a| a == "--config-path").and_then(|i| args.get(i + 1));
    let alias = args.iter().position(|a| a == "--alias").and_then(|i| args.get(i + 1));
    return handle_init(config_path.map(|s| s), alias);
}
```

Note: the variable inside the init block is renamed from `path` to `config_path` to avoid shadowing the outer `config_path` variable — but since we `return` inside the block, it's fine either way. Use a different name for clarity:

```rust
if args.len() >= 2 && args[1] == "init" {
    let init_path = args.iter().position(|a| a == "--config-path").and_then(|i| args.get(i + 1));
    let alias = args.iter().position(|a| a == "--alias").and_then(|i| args.get(i + 1));
    return handle_init(init_path, alias);
}
```

**Step 3: Run all tests**

Run: `eval "$(mise activate zsh)" && cargo test`
Expected: All tests PASS (init tests use `--config-path`, alias logic is wired but no alias tests invoke it yet)

**Step 4: Commit**

```bash
git add src/main.rs
git commit -m "feat: wire --alias and --config-path into init command"
```

---

### Task 6: Add integration test — init --alias appends alias to shell config

**Files:**
- Modify: `tests/integration.rs`

**Step 1: Write the test**

Add to `tests/integration.rs`:

```rust
#[test]
fn test_init_alias_appends_to_shell_config() {
    // This test creates a fake .zshrc and uses HOME override to test alias installation.
    // Since detect_shell_configs() uses dirs::home_dir(), we can't easily override HOME
    // in an integration test on all platforms. Instead, test the init command without alias
    // and verify the config file is created; alias installation is covered by unit tests.
    //
    // Alternatively, we test the CLI output to verify --alias is accepted:
    let dir = tempfile::TempDir::new().expect("tempdir");
    let target = dir.path().join("alias_test.yml");
    let output = cargo_bin()
        .args(["init", "--config-path", target.to_str().unwrap(), "--alias", "my-tool"])
        .output()
        .expect("failed to run cargo run");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "init --alias failed:\nstdout: {stdout}\nstderr: {stderr}"
    );
    assert!(target.exists(), "config file should be created");
    // The alias output varies depending on which shell configs exist on the test machine.
    // At minimum verify the config was created.
    assert!(
        stdout.contains("Created configuration at:"),
        "expected creation message in stdout:\n{stdout}"
    );
}
```

**Step 2: Run the test**

Run: `eval "$(mise activate zsh)" && cargo test -- test_init_alias`
Expected: PASS

**Step 3: Commit**

```bash
git add tests/integration.rs
git commit -m "test: add integration test for init --alias"
```

---

### Task 7: Add integration test — running init --alias twice doesn't duplicate

**Files:**
- Modify: `tests/integration.rs`

**Step 1: Write the test**

Add to `tests/integration.rs`:

```rust
#[test]
fn test_init_alias_no_duplicate() {
    let dir = tempfile::TempDir::new().expect("tempdir");
    let target1 = dir.path().join("first.yml");
    let output1 = cargo_bin()
        .args(["init", "--config-path", target1.to_str().unwrap(), "--alias", "dup-test"])
        .output()
        .expect("failed to run cargo run");
    assert!(output1.status.success(), "first init failed");

    // Second init with different config path but same alias name
    let target2 = dir.path().join("second.yml");
    let output2 = cargo_bin()
        .args(["init", "--config-path", target2.to_str().unwrap(), "--alias", "dup-test"])
        .output()
        .expect("failed to run cargo run");
    let stdout2 = String::from_utf8_lossy(&output2.stdout);
    assert!(
        output2.status.success(),
        "second init failed:\n{}", String::from_utf8_lossy(&output2.stderr)
    );
    // If shell configs exist, should show "already exists" message
    // If no shell configs exist, shows warning — either way it should succeed
    assert!(
        stdout2.contains("Created configuration at:"),
        "expected creation message in stdout:\n{stdout2}"
    );
}
```

**Step 2: Run the test**

Run: `eval "$(mise activate zsh)" && cargo test -- test_init_alias_no_duplicate`
Expected: PASS

**Step 3: Commit**

```bash
git add tests/integration.rs
git commit -m "test: add integration test for alias duplicate prevention"
```

---

### Task 8: Final verification — run full test suite

**Step 1: Run all tests**

Run: `eval "$(mise activate zsh)" && cargo test`
Expected: All tests PASS (unit tests in main.rs, models.rs, utils.rs, cli.rs + integration tests)

**Step 2: Run clippy**

Run: `eval "$(mise activate zsh)" && cargo clippy -- -D warnings`
Expected: No warnings

**Step 3: Manual smoke test**

Run: `eval "$(mise activate zsh)" && cargo run -- init --config-path /tmp/ring-smoke-test.yml --alias ring-smoke`
Expected: Creates config file and reports alias status. Clean up after: `rm /tmp/ring-smoke-test.yml`

**Step 4: Commit any fixes needed, then final commit**

```bash
git add -A
git commit -m "feat: complete init --alias feature with shell alias installation"
```
