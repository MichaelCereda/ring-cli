# Ring-CLI

Ring-CLI is a versatile command-line interface tool powered by YAML configurations, allowing you to define and execute custom commands, subcommands, and HTTP requests. 

## Features

- **Custom Command Definition**: Define your commands, subcommands, and their associated flags using simple YAML files.
- **Flexible Execution**: Execute shell commands, or HTTP requests directly from the CLI.
- **Dynamic Placeholder Replacement**: Use placeholders in your commands and replace them with flag values during execution.
- **Configurable Base Directory**: Run your commands from a specified base directory.
- **Verbose & Quiet Modes**: Toggle verbose outputs or suppress error messages using global flags.
- **Recursive Subcommand Support**: Nest subcommands recursively for complex command structures.
- **HTTP Request Execution**: Execute HTTP requests with custom headers and body content.
- **Auto-Validation**: Commands and subcommands are automatically validated upon loading.

## Parameters

| Parameter        | Short Form | Description                                                          |
|------------------|------------|----------------------------------------------------------------------|
| `--quiet`        | `-q`      | Suppress error messages.                                             |
| `--verbose`      | `-v`      | Print verbose output.                                                |
| `--config=PATH`  | `-c`      | Path to a custom configuration file or directory.                    |
| `--base-dir=PATH`| `-b`      | Base directory from which relative paths in commands will be resolved|

## Example Configuration

Here's a simple example of a YAML configuration:

```yaml
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
  httpbin:
    description: "Perform operations with httpbin"
    subcommands:
      get:
        description: "Make a GET request to httpbin"
        cmd:
          http:
            method: "GET"
            url: "https://httpbin.org/get"
      post:
        description: "Make a POST request to httpbin"
        flags:
          - name: "data"
            short: "d"
            description: "Data to POST"
        cmd:
          http:
            method: "POST"
            url: "https://httpbin.org/post"
            body: "${{data}}"
```

With this configuration, you can run commands like:

- `ring-cli greet --name=John`
- `ring-cli httpbin get`
- `ring-cli httpbin post --data="Hello"`

## Alias Examples

You can create aliases for frequently used configurations or commands for different shells:

### Bash or Zsh:

```bash
alias my-cli='ring-cli -c /path/to/your/config.yml -b /your/base/directory'
```

### Fish:

```fish
alias my-cli 'ring-cli -c /path/to/your/config.yml -b /your/base/directory'
```

### PowerShell:

```powershell
Set-Alias -Name my-cli -Value 'ring-cli -c /path/to/your/config.yml -b /your/base/directory'
```

After setting up the alias, you can use `my-cli` in place of the full `ring-cli` command with the specified configuration and base directory.

## Wrapping Up

Ring-CLI offers an easy and flexible way to extend your command-line capabilities. By writing simple YAML configurations, you can create and customize your commands to fit your needs. Whether it's running local scripts, making HTTP requests, or anything in between, Ring-CLI has got you covered.
