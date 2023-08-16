use serde_derive::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
struct Configuration {
    version: String,
    description: String,
    slug: String,
    commands: std::collections::HashMap<String, Command>,
}

#[derive(Debug, Deserialize, Serialize)]
struct Command {
    description: String,
    flags: Vec<Flag>,
    cmd: Option<CmdType>,
    subcommands: Option<std::collections::HashMap<String, Command>>,
}
impl Command {
    fn validate(&self) -> Result<(), String> {
        match (&self.cmd, &self.subcommands) {
            (Some(_), Some(_)) => {
                return Err("Only 'cmd' or 'subcommands' should be present, not both.".to_string())
            }
            (None, None) => {
                return Err("Either 'cmd' or 'subcommands' must be present.".to_string())
            }
            _ => (),
        }

        if let Some(subcommands) = &self.subcommands {
            for (_, sub_cmd) in subcommands {
                sub_cmd.validate()?; // Recursive call
            }
        }

        Ok(())
    }
}

#[derive(Debug, Deserialize, Serialize)]
struct Flag {
    name: String,
    #[serde(default)]
    short: Option<String>,
    description: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(untagged)]
enum CmdType {
    Http { http: Http },
    Run { run: Vec<String> },
}

#[derive(Debug, Deserialize, Serialize)]
struct Http {
    method: String,
    url: String,
    headers: Option<HashMap<String, String>>,
    #[serde(default)]
    body: Option<String>,
}

use dirs;
use std::{collections::HashMap, fs};

fn replace_placeholders<'a>(
    template: &str,
    flags: &'a clap::ArgMatches<'a>,
    verbose: bool,
) -> String {
    let mut result = template.to_string();
    for (flag_name, values) in flags.args.iter() {
        let flag_value = values.vals[0].to_str().unwrap_or_default();
        if verbose {
            println!("Replacing placeholder for {}: {}", flag_name, flag_value);
        }
        result = result.replace(&format!("${{{{{}}}}}", flag_name), flag_value);
    }
    result
}

fn load_configurations(config_path: Option<&str>) -> Result<Vec<Configuration>, Box<dyn std::error::Error>> {
    let mut configurations = Vec::new();

    // Set the default config directory to ~/.ring-cli/configurations
    let default_config_dir = dirs::home_dir()
        .ok_or("Unable to determine home directory")?
        .join(".ring-cli/configurations");

    // If a custom config path is provided, use it. Otherwise, use the default directory.
    let config_dir = if let Some(path) = config_path {
        std::path::PathBuf::from(path)
    } else {
        default_config_dir
    };
    
    if config_dir.is_file() {
        let content = fs::read_to_string(&config_dir)?;
        let config: Configuration = serde_yaml::from_str(&content)?;
        configurations.push(config);
    } else if config_dir.is_dir() {
        let paths = fs::read_dir(config_dir)?;
        for path in paths {
            let content = fs::read_to_string(path?.path())?;
            let config: Configuration = serde_yaml::from_str(&content)?;
            configurations.push(config);
        }
    } else {
        return Err(Box::from("Provided config path is neither a file nor a directory"));
    }

    for config in &configurations {
        for (_, cmd) in &config.commands {
            cmd.validate()?;  // Validate each command after loading
        }
    }

    Ok(configurations)
}

use clap::{App, Arg, SubCommand};

fn add_subcommands_to_cli<'a>(
    command: &'a Command,
    cmd_subcommand: clap::App<'a, 'a>,
) -> clap::App<'a, 'a> {
    let mut updated_subcommand = cmd_subcommand;
    if let Some(subcommands) = &command.subcommands {
        for (sub_name, sub_cmd) in subcommands {
            let mut sub_cli = SubCommand::with_name(sub_name).about(sub_cmd.description.as_str());
            for flag in &sub_cmd.flags {
                let mut arg = Arg::with_name(&flag.name)
                    .long(&flag.name)
                    .help(&flag.description)
                    .takes_value(true);
                if let Some(short_form) = &flag.short {
                    arg = arg.short(short_form);
                }
                sub_cli = sub_cli.arg(arg);
            }
            sub_cli = add_subcommands_to_cli(&sub_cmd, sub_cli);
            updated_subcommand = updated_subcommand.subcommand(sub_cli);
        }
    }
    updated_subcommand
}

fn build_cli_from_configs(configs: &Vec<Configuration>) -> App {
    let mut app = App::new("ring-cli")
        .version("1.0")
        .about("Ring CLI Tool powered by YAML configurations")
        .arg(
            Arg::with_name("quiet")
                .short("q")
                .long("quiet")
                .help("Suppress error messages"),
        )
        .arg(
            Arg::with_name("verbose")
                .short("v")
                .long("verbose")
                .help("Print verbose output"),
        )
        .arg(Arg::with_name("config")
             .short("c")
             .long("config")
             .value_name("PATH")
             .help("Path to a custom configuration file or directory")
             .takes_value(true));
    for config in configs {
        let mut subcommand = SubCommand::with_name(&config.slug)
            .about(config.description.as_str())
            .version(config.version.as_str());
        for (cmd_name, cmd) in &config.commands {
            let mut cmd_subcommand =
                SubCommand::with_name(cmd_name).about(cmd.description.as_str());
            for flag in &cmd.flags {
                let mut arg = Arg::with_name(&flag.name)
                    .long(&flag.name)
                    .help(&flag.description)
                    .takes_value(true);
                if let Some(short_form) = &flag.short {
                    arg = arg.short(short_form);
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
use std::process::Command as ShellCommand;

fn run_shell_commands(
    commands: &Vec<String>,
    flags: &clap::ArgMatches,
    verbose: bool,
) -> Result<String, String> {
    let mut output_text = String::new();
    for cmd in commands {
        let replaced_cmd = replace_placeholders(cmd, flags, verbose);

        // Running the command using a shell
        let output = ShellCommand::new("sh")
            .arg("-c")
            .arg(&replaced_cmd)
            .output()
            .map_err(|e| format!("Failed to run command '{}': {}", cmd, e))?;

        if output.status.success() {
            output_text.push_str(&String::from_utf8_lossy(&output.stdout));
        } else {
            return Err(String::from_utf8_lossy(&output.stderr).to_string());
        }
    }
    Ok(output_text)
}
use reqwest;

async fn execute_http_request<'a>(
    http: &Http,
    flags: &'a clap::ArgMatches<'a>,
) -> Result<String, String> {
    let client = reqwest::Client::new();

    let replace_with_flag_values = |template: &str| -> String {
        let mut result = template.to_string();
        for (flag_name, values) in flags.args.iter() {
            let flag_value = values.vals[0].to_str().unwrap_or_default();
            result = result.replace(&format!("${{{{{}}}}}", flag_name), flag_value);
        }
        result
    };

    let url = replace_with_flag_values(&http.url);
    let body = if let Some(ref body_content) = &http.body {
        Some(replace_with_flag_values(body_content))
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
        _ => return Err(format!("Unsupported HTTP method '{}'", http.method)),
    };

    // Adding headers if they exist
    let mut request_with_headers = request_builder;
    if let Some(header_map) = &http.headers {
        for (header_name, header_value) in header_map.iter() {
            request_with_headers = request_with_headers.header(header_name, header_value);
        }
    }

    let response = request_with_headers
        .send()
        .await
        .map_err(|e| format!("Failed to send request: {}", e))?;

    let text = response
        .text()
        .await
        .map_err(|e| format!("Failed to read response: {}", e))?;

    Ok(text)
}

fn execute_command(
    command: &Command,
    cmd_matches: &clap::ArgMatches,
    verbose: bool,
) -> Result<(), String> {
    if verbose {
        println!("Executing command with flags: {:?}", cmd_matches.args);
    }
    if let Some(actual_cmd) = &command.cmd {
        match actual_cmd {
            CmdType::Http { http } => {
                match tokio::runtime::Runtime::new()
                    .unwrap()
                    .block_on(execute_http_request(http, &cmd_matches))
                {
                    Ok(output) => println!("{}", output),
                    Err(e) => eprintln!("Error executing HTTP request: {}", e),
                }
            }
            CmdType::Run { run } => {
                match run_shell_commands(run, cmd_matches, verbose) {
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
                execute_command(sub_cmd, sub_cmd_matches, verbose)?;
            }
        }
    }
    Ok(())
}

fn main() {
    let config_path = std::env::args().find(|arg| arg.starts_with("--config=") || arg == "-c")
    .and_then(|arg| arg.split('=').nth(1).map(String::from));

let configurations = load_configurations(config_path.as_deref()).unwrap_or_else(|e| {
    eprintln!("Error loading configurations: {}", e);
    std::process::exit(1);
});

let matches = build_cli_from_configs(&configurations).get_matches();



    let is_quiet = matches.is_present("quiet");
    let is_verbose = matches.is_present("verbose");
    for config in &configurations {
        if let Some(submatches) = matches.subcommand_matches(&config.slug) {
            for (cmd_name, cmd) in &config.commands {
                if let Some(cmd_matches) = submatches.subcommand_matches(cmd_name) {
                    if let Err(e) = execute_command(cmd, cmd_matches, is_verbose) {
                        if !is_quiet {
                            eprintln!("Error: {}", e);
                        }
                        std::process::exit(1);
                    }
                }
            }
        }
    }
}
