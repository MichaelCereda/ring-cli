use std::fs;
use std::path::PathBuf;

#[derive(Clone, Copy)]
pub(crate) enum ShellKind {
    BashZsh,
    Fish,
    PowerShell,
}

pub(crate) struct ShellConfig {
    pub path: PathBuf,
    pub kind: ShellKind,
    pub display_name: &'static str,
}

pub(crate) fn alias_line_bash_zsh(alias_name: &str) -> String {
    format!("{alias_name}() {{ stampo --alias-mode {alias_name} \"$@\"; }} # stampo")
}

pub(crate) fn alias_line_fish(alias_name: &str) -> String {
    format!("function {alias_name}; stampo --alias-mode {alias_name} $argv; end # stampo")
}

pub(crate) fn alias_line_powershell(alias_name: &str) -> String {
    format!("function {alias_name} {{ stampo --alias-mode {alias_name} @args }} # stampo")
}

pub(crate) fn alias_exists(file_content: &str, alias_name: &str, kind: ShellKind) -> bool {
    match kind {
        ShellKind::BashZsh => {
            // Detect both old alias format and new function format
            file_content.contains(&format!("alias {alias_name}="))
                || file_content.contains(&format!("{alias_name}()"))
        }
        ShellKind::Fish => {
            file_content.contains(&format!("alias {alias_name} "))
                || file_content.contains(&format!("function {alias_name};"))
        }
        ShellKind::PowerShell => file_content.contains(&format!("function {alias_name}")),
    }
}

pub(crate) fn detect_shell_configs() -> Vec<ShellConfig> {
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

pub(crate) fn install_alias(alias_name: &str) -> Result<(), anyhow::Error> {
    let shells = detect_shell_configs();
    if shells.is_empty() {
        eprintln!("Warning: No shell config files found. Add the alias manually:");
        eprintln!("  Bash/Zsh: {}", alias_line_bash_zsh(alias_name));
        eprintln!("  Fish:     {}", alias_line_fish(alias_name));
        eprintln!("  PowerShell: {}", alias_line_powershell(alias_name));
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
            ShellKind::BashZsh => alias_line_bash_zsh(alias_name),
            ShellKind::Fish => alias_line_fish(alias_name),
            ShellKind::PowerShell => alias_line_powershell(alias_name),
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

pub(crate) fn install_update_check(alias_name: &str) -> Result<(), anyhow::Error> {
    let shells = detect_shell_configs();
    for shell in &shells {
        let content = fs::read_to_string(&shell.path)?;
        let marker = format!("# stampo-update-check:{alias_name}");
        if content.contains(&marker) {
            continue;
        }
        let hook = format!("stampo --check-updates {alias_name} {marker}");
        let mut file = fs::OpenOptions::new().append(true).open(&shell.path)?;
        use std::io::Write;
        writeln!(file, "{}", hook)?;
    }
    Ok(())
}

pub(crate) fn clean_alias_lines(content: &str, alias_name: &str, kind: ShellKind) -> String {
    let stampo_marker = "# stampo";
    let completion_marker = format!("# stampo-completions:{alias_name}");
    let update_marker = format!("# stampo-update-check:{alias_name}");

    let filtered: Vec<&str> = content
        .lines()
        .filter(|line| {
            let is_alias_line = match kind {
                ShellKind::BashZsh => {
                    (line.contains(&format!("alias {alias_name}="))
                        || line.contains(&format!("{alias_name}()")))
                        && line.contains(stampo_marker)
                }
                ShellKind::Fish => {
                    (line.contains(&format!("alias {alias_name} "))
                        || line.contains(&format!("function {alias_name};")))
                        && line.contains(stampo_marker)
                }
                ShellKind::PowerShell => {
                    line.contains(&format!("function {alias_name}")) && line.contains(stampo_marker)
                }
            };
            let is_completion_line = line.contains(&completion_marker);
            let is_update_line = line.contains(&update_marker);
            !is_alias_line && !is_completion_line && !is_update_line
        })
        .collect();

    let mut result = filtered.join("\n");
    if content.ends_with('\n') {
        result.push('\n');
    }
    result
}

pub(crate) fn clean_alias_from_shells(alias_name: &str) -> Result<(), anyhow::Error> {
    let shells = detect_shell_configs();
    for shell in &shells {
        let content = fs::read_to_string(&shell.path)?;
        let cleaned = clean_alias_lines(&content, alias_name, shell.kind);
        if cleaned != content {
            fs::write(&shell.path, &cleaned)?;
        }
    }
    Ok(())
}

pub(crate) fn remove_update_check(alias_name: &str) -> Result<(), anyhow::Error> {
    let shells = detect_shell_configs();
    let marker = format!("# stampo-update-check:{alias_name}");
    for shell in &shells {
        let content = fs::read_to_string(&shell.path)?;
        if !content.contains(&marker) {
            continue;
        }
        let filtered: String = content
            .lines()
            .filter(|line| !line.contains(&marker))
            .collect::<Vec<_>>()
            .join("\n");
        // Preserve trailing newline
        let filtered = if content.ends_with('\n') {
            format!("{filtered}\n")
        } else {
            filtered
        };
        fs::write(&shell.path, filtered)?;
    }
    Ok(())
}

pub(crate) fn install_completions(alias_name: &str) -> Result<(), anyhow::Error> {
    let shells = detect_shell_configs();
    for shell in &shells {
        let content = fs::read_to_string(&shell.path)?;
        let completion_marker = format!("# stampo-completions:{alias_name}");
        if content.contains(&completion_marker) {
            continue;
        }
        let hook = match shell.kind {
            ShellKind::BashZsh => {
                if shell.display_name.contains("zsh") {
                    format!(
                        "eval \"$(stampo --generate-completions zsh {alias_name})\" {completion_marker}"
                    )
                } else {
                    format!(
                        "eval \"$(stampo --generate-completions bash {alias_name})\" {completion_marker}"
                    )
                }
            }
            ShellKind::Fish => {
                format!(
                    "stampo --generate-completions fish {alias_name} | source {completion_marker}"
                )
            }
            ShellKind::PowerShell => {
                format!(
                    "stampo --generate-completions powershell {alias_name} | Invoke-Expression {completion_marker}"
                )
            }
        };
        let mut file = fs::OpenOptions::new().append(true).open(&shell.path)?;
        use std::io::Write;
        writeln!(file, "{}", hook)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bash_alias_line() {
        let line = alias_line_bash_zsh("my-tool");
        assert_eq!(line, "my-tool() { stampo --alias-mode my-tool \"$@\"; } # stampo");
    }

    #[test]
    fn test_fish_alias_line() {
        let line = alias_line_fish("my-tool");
        assert_eq!(line, "function my-tool; stampo --alias-mode my-tool $argv; end # stampo");
    }

    #[test]
    fn test_powershell_alias_line() {
        let line = alias_line_powershell("my-tool");
        assert_eq!(
            line,
            "function my-tool { stampo --alias-mode my-tool @args } # stampo"
        );
    }

    #[test]
    fn test_alias_already_exists_bash() {
        // New function format
        let content = "# my stuff\nmy-tool() { stampo --alias-mode my-tool \"$@\"; } # stampo\n";
        assert!(alias_exists(content, "my-tool", ShellKind::BashZsh));
        assert!(!alias_exists(content, "other-tool", ShellKind::BashZsh));
        // Old alias format still detected
        let old = "alias my-tool='stampo --alias-mode my-tool' # stampo\n";
        assert!(alias_exists(old, "my-tool", ShellKind::BashZsh));
    }

    #[test]
    fn test_alias_already_exists_fish() {
        let content = "function my-tool; stampo --alias-mode my-tool $argv; end # stampo\n";
        assert!(alias_exists(content, "my-tool", ShellKind::Fish));
        assert!(!alias_exists(content, "other-tool", ShellKind::Fish));
        // Old alias format still detected
        let old = "alias my-tool 'stampo --alias-mode my-tool' # stampo\n";
        assert!(alias_exists(old, "my-tool", ShellKind::Fish));
    }

    #[test]
    fn test_alias_already_exists_powershell() {
        let content = "function my-tool { stampo --alias-mode my-tool @args } # stampo\n";
        assert!(alias_exists(content, "my-tool", ShellKind::PowerShell));
        assert!(!alias_exists(content, "other-tool", ShellKind::PowerShell));
    }

    #[test]
    fn test_clean_alias_lines_removes_stampo_entries() {
        let content = "# my stuff\nos() { stampo --alias-mode os \"$@\"; } # stampo\neval \"$(stampo --generate-completions zsh os)\" # stampo-completions:os\nstampo --check-updates os # stampo-update-check:os\nexport PATH=$HOME/bin:$PATH\n";
        let cleaned = clean_alias_lines(content, "os", ShellKind::BashZsh);
        assert_eq!(cleaned, "# my stuff\nexport PATH=$HOME/bin:$PATH\n");
    }

    #[test]
    fn test_clean_alias_lines_removes_old_alias_format() {
        let content = "alias os='stampo --alias-mode os' # stampo\neval \"$(stampo --generate-completions zsh os)\" # stampo-completions:os\n";
        let cleaned = clean_alias_lines(content, "os", ShellKind::BashZsh);
        assert_eq!(cleaned, "\n");
    }

    #[test]
    fn test_clean_alias_lines_preserves_other_aliases() {
        let content = "os() { stampo --alias-mode os \"$@\"; } # stampo\nalias other='something'\n";
        let cleaned = clean_alias_lines(content, "os", ShellKind::BashZsh);
        assert_eq!(cleaned, "alias other='something'\n");
    }

    #[test]
    fn test_clean_alias_lines_ignores_non_stampo_function() {
        let content = "os() { my-custom-command; }\n";
        let cleaned = clean_alias_lines(content, "os", ShellKind::BashZsh);
        assert_eq!(cleaned, "os() { my-custom-command; }\n");
    }
}
