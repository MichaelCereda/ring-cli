# Init --alias Feature Design

## Context

ring-cli's `init` command creates a starter YAML config. Users often create shell aliases to bind `ring-cli -c /path/to/config.yml` to a short name (documented in README). The `--alias` flag automates this.

Also renames the existing `--path` parameter to `--config-path` for clarity.

## CLI Usage

```
ring-cli init [--config-path ./config.yml] [--alias my-tool]
```

## Behavior

1. Create the config YAML (existing behavior)
2. If `--alias` is provided, resolve the absolute path to the config file
3. Detect which shell config files exist on disk
4. For each detected shell config, append the alias with a `# ring-cli` comment marker
5. Skip if the alias already exists in that file
6. Print which files were modified

## Shell Config Detection

Only write to files that already exist on disk. Don't create shell configs.

| Shell | Config file | Alias syntax |
|---|---|---|
| bash | `~/.bashrc` | `alias my-tool='ring-cli -c /abs/path/config.yml' # ring-cli` |
| zsh | `~/.zshrc` | `alias my-tool='ring-cli -c /abs/path/config.yml' # ring-cli` |
| fish | `~/.config/fish/config.fish` | `alias my-tool 'ring-cli -c /abs/path/config.yml' # ring-cli` |
| PowerShell (Linux/macOS) | `~/.config/powershell/Microsoft.PowerShell_profile.ps1` | `function my-tool { ring-cli -c /abs/path/config.yml @args } # ring-cli` |
| PowerShell (Windows) | `~/Documents/PowerShell/Microsoft.PowerShell_profile.ps1` | same |

If no shell configs found, print a warning with manual alias instructions for all shells.

## Duplicate Detection

Before appending, check if the file contains `alias my-tool=` (bash/zsh), `alias my-tool ` (fish), or `function my-tool` (PowerShell). If found, skip and print a message.

## Output

```
$ ring-cli init --alias my-tool
Created configuration at: /Users/me/.ring-cli/configurations/example.yml
Added alias 'my-tool' to:
  ~/.zshrc
  ~/.bashrc
Restart your terminal or run 'source ~/.zshrc' to use 'my-tool'.
```

## Error Cases

- `--alias` without a value: error message
- Config file path can't be resolved to absolute: error
- No shell configs found: warning with manual instructions

## Parameter Rename

Rename `--path` to `--config-path` in init command and all tests.

## Tests

- Unit test: generate correct alias line for each shell format
- Integration test: init --alias appends alias to a shell config
- Integration test: running twice doesn't duplicate the alias
- Update existing tests that use `--path` to use `--config-path`

## Files Changed

- Modify: `src/main.rs` (init args, alias logic)
- Modify: `tests/integration.rs` (update --path to --config-path, add alias tests)
