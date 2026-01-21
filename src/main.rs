//! shclap - Clap-style argument parsing for shell scripts.

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use shclap::{
    generate_error_output, generate_help, generate_help_output, generate_output, generate_version,
    generate_version_output, parse_args, Config, ParseOutcome,
};

/// Clap-style argument parsing for shell scripts.
#[derive(Parser, Debug)]
#[command(name = "shclap", version, about, disable_help_subcommand = true)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Parse script arguments and output environment variables
    Parse {
        /// JSON configuration for the target script
        #[arg(long)]
        config: String,

        /// Application name (overrides config 'name' field)
        #[arg(long)]
        name: Option<String>,

        /// Environment variable prefix (overrides config)
        #[arg(long)]
        prefix: Option<String>,

        /// Arguments to parse for the target script
        #[arg(last = true)]
        args: Vec<String>,
    },

    /// Print help text for the target script
    Help {
        /// JSON configuration for the target script
        #[arg(long)]
        config: String,

        /// Application name (overrides config 'name' field)
        #[arg(long)]
        name: Option<String>,
    },

    /// Print version of the target script
    Version {
        /// JSON configuration for the target script
        #[arg(long)]
        config: String,

        /// Application name (overrides config 'name' field)
        #[arg(long)]
        name: Option<String>,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Parse {
            config,
            name,
            prefix,
            args,
        } => {
            // Handle config parsing errors
            let cfg = match Config::from_json(&config) {
                Ok(c) => c,
                Err(e) => {
                    return output_error(&format!("failed to parse JSON config: {}", e));
                }
            };

            // Handle validation errors
            if let Err(e) = cfg.validate() {
                return output_error(&e.to_string());
            }

            // Determine effective name: CLI --name takes priority over config name
            let effective_name = match (name.as_deref(), cfg.name.as_deref()) {
                (Some(cli_name), _) => cli_name,
                (None, Some(config_name)) => config_name,
                (None, None) => {
                    return output_error(
                        "no application name provided: use --name or set 'name' in config",
                    );
                }
            };

            let effective_prefix = prefix.as_deref().unwrap_or_else(|| cfg.effective_prefix());

            // Handle parse result
            match parse_args(&cfg, &args, effective_name) {
                ParseOutcome::Success(result) => {
                    let path = generate_output(
                        &result.values,
                        effective_prefix,
                        result.subcommand.as_deref(),
                    )
                    .context("failed to generate output file")?;
                    println!("{}", path.display());
                }
                ParseOutcome::Help(help_text) => {
                    let path = generate_help_output(&help_text)
                        .context("failed to generate help output file")?;
                    println!("{}", path.display());
                }
                ParseOutcome::Version(version_text) => {
                    let path = generate_version_output(&version_text)
                        .context("failed to generate version output file")?;
                    println!("{}", path.display());
                }
                ParseOutcome::Error(error_msg) => {
                    return output_error(&error_msg);
                }
            }
        }
        Commands::Help { config, name } => {
            let cfg = Config::from_json(&config).context("failed to parse config JSON")?;

            // Determine effective name: CLI --name takes priority over config name
            let effective_name = match (name.as_deref(), cfg.name.as_deref()) {
                (Some(cli_name), _) => cli_name.to_string(),
                (None, Some(config_name)) => config_name.to_string(),
                (None, None) => {
                    anyhow::bail!(
                        "no application name provided: use --name or set 'name' in config"
                    );
                }
            };

            print!("{}", generate_help(&cfg, &effective_name));
        }
        Commands::Version { config, name } => {
            let cfg = Config::from_json(&config).context("failed to parse config JSON")?;

            // Determine effective name: CLI --name takes priority over config name
            let effective_name = match (name.as_deref(), cfg.name.as_deref()) {
                (Some(cli_name), _) => cli_name.to_string(),
                (None, Some(config_name)) => config_name.to_string(),
                (None, None) => {
                    anyhow::bail!(
                        "no application name provided: use --name or set 'name' in config"
                    );
                }
            };

            print!("{}", generate_version(&cfg, &effective_name));
        }
    }

    Ok(())
}

/// Output an error file path and return Ok.
/// Falls back to stderr + exit 1 if file creation fails.
fn output_error(message: &str) -> Result<()> {
    match generate_error_output(message) {
        Ok(path) => {
            println!("{}", path.display());
            Ok(())
        }
        Err(e) => {
            eprintln!("shclap: {}", message);
            eprintln!("shclap: also failed to create error output file: {}", e);
            std::process::exit(1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn test_parse_subcommand_parses_config() {
        let cli = Cli::try_parse_from(["shclap", "parse", "--config", r#"{"name":"test"}"#, "--"])
            .unwrap();

        match cli.command {
            Commands::Parse {
                config,
                name,
                prefix,
                args,
            } => {
                assert_eq!(config, r#"{"name":"test"}"#);
                assert!(name.is_none());
                assert!(prefix.is_none());
                assert!(args.is_empty());
            }
            _ => panic!("Expected Parse command"),
        }
    }

    #[test]
    fn test_parse_subcommand_parses_prefix() {
        let cli = Cli::try_parse_from([
            "shclap",
            "parse",
            "--config",
            r#"{"name":"test"}"#,
            "--prefix",
            "MYAPP_",
            "--",
        ])
        .unwrap();

        match cli.command {
            Commands::Parse { name, prefix, .. } => {
                assert!(name.is_none());
                assert_eq!(prefix, Some("MYAPP_".to_string()));
            }
            _ => panic!("Expected Parse command"),
        }
    }

    #[test]
    fn test_parse_subcommand_parses_name() {
        let cli = Cli::try_parse_from([
            "shclap", "parse", "--config", r#"{}"#, "--name", "myapp", "--",
        ])
        .unwrap();

        match cli.command {
            Commands::Parse { name, .. } => {
                assert_eq!(name, Some("myapp".to_string()));
            }
            _ => panic!("Expected Parse command"),
        }
    }

    #[test]
    fn test_parse_subcommand_parses_args() {
        let cli = Cli::try_parse_from([
            "shclap",
            "parse",
            "--config",
            r#"{"name":"test"}"#,
            "--",
            "-v",
            "--output",
            "file.txt",
            "input.txt",
        ])
        .unwrap();

        match cli.command {
            Commands::Parse { args, .. } => {
                assert_eq!(args, vec!["-v", "--output", "file.txt", "input.txt"]);
            }
            _ => panic!("Expected Parse command"),
        }
    }

    #[test]
    fn test_parse_subcommand_requires_config() {
        let result = Cli::try_parse_from(["shclap", "parse", "--"]);
        assert!(result.is_err());
    }

    #[test]
    fn test_help_subcommand() {
        let cli = Cli::try_parse_from([
            "shclap",
            "help",
            "--config",
            r#"{"name":"test","description":"A test"}"#,
        ])
        .unwrap();

        match cli.command {
            Commands::Help { config, name } => {
                assert_eq!(config, r#"{"name":"test","description":"A test"}"#);
                assert!(name.is_none());
            }
            _ => panic!("Expected Help command"),
        }
    }

    #[test]
    fn test_help_subcommand_with_name() {
        let cli = Cli::try_parse_from([
            "shclap",
            "help",
            "--config",
            r#"{"description":"A test"}"#,
            "--name",
            "myapp",
        ])
        .unwrap();

        match cli.command {
            Commands::Help { config, name } => {
                assert_eq!(config, r#"{"description":"A test"}"#);
                assert_eq!(name, Some("myapp".to_string()));
            }
            _ => panic!("Expected Help command"),
        }
    }

    #[test]
    fn test_version_subcommand() {
        let cli = Cli::try_parse_from([
            "shclap",
            "version",
            "--config",
            r#"{"name":"test","version":"1.0.0"}"#,
        ])
        .unwrap();

        match cli.command {
            Commands::Version { config, name } => {
                assert_eq!(config, r#"{"name":"test","version":"1.0.0"}"#);
                assert!(name.is_none());
            }
            _ => panic!("Expected Version command"),
        }
    }

    #[test]
    fn test_version_subcommand_with_name() {
        let cli = Cli::try_parse_from([
            "shclap",
            "version",
            "--config",
            r#"{"version":"1.0.0"}"#,
            "--name",
            "myapp",
        ])
        .unwrap();

        match cli.command {
            Commands::Version { config, name } => {
                assert_eq!(config, r#"{"version":"1.0.0"}"#);
                assert_eq!(name, Some("myapp".to_string()));
            }
            _ => panic!("Expected Version command"),
        }
    }

    #[test]
    fn test_cli_requires_subcommand() {
        let result = Cli::try_parse_from(["shclap"]);
        assert!(result.is_err());
    }

    #[test]
    fn test_cli_help() {
        // Verify the command can generate help without panicking
        Cli::command().debug_assert();
    }

    #[test]
    fn test_prefix_priority_cli_overrides_config() {
        let cli = Cli::try_parse_from([
            "shclap",
            "parse",
            "--config",
            r#"{"name":"test","prefix":"CONFIG_"}"#,
            "--prefix",
            "CLI_",
            "--",
        ])
        .unwrap();

        match cli.command {
            Commands::Parse {
                config,
                name: _,
                prefix,
                args: _,
            } => {
                let cfg = Config::from_json(&config).unwrap();
                let effective = prefix.as_deref().unwrap_or_else(|| cfg.effective_prefix());
                assert_eq!(effective, "CLI_");
            }
            _ => panic!("Expected Parse command"),
        }
    }

    #[test]
    fn test_prefix_priority_config_when_no_cli() {
        let cli = Cli::try_parse_from([
            "shclap",
            "parse",
            "--config",
            r#"{"name":"test","prefix":"CONFIG_"}"#,
            "--",
        ])
        .unwrap();

        match cli.command {
            Commands::Parse {
                config,
                name: _,
                prefix,
                args: _,
            } => {
                let cfg = Config::from_json(&config).unwrap();
                let effective = prefix.as_deref().unwrap_or_else(|| cfg.effective_prefix());
                assert_eq!(effective, "CONFIG_");
            }
            _ => panic!("Expected Parse command"),
        }
    }

    #[test]
    fn test_prefix_priority_default_when_neither_set() {
        let cli = Cli::try_parse_from(["shclap", "parse", "--config", r#"{"name":"test"}"#, "--"])
            .unwrap();

        match cli.command {
            Commands::Parse {
                config,
                name: _,
                prefix,
                args: _,
            } => {
                let cfg = Config::from_json(&config).unwrap();
                let effective = prefix.as_deref().unwrap_or_else(|| cfg.effective_prefix());
                assert_eq!(effective, "SHCLAP_");
            }
            _ => panic!("Expected Parse command"),
        }
    }

    #[test]
    fn test_name_priority_cli_overrides_config() {
        let cli = Cli::try_parse_from([
            "shclap",
            "parse",
            "--config",
            r#"{"name":"config_name"}"#,
            "--name",
            "cli_name",
            "--",
        ])
        .unwrap();

        match cli.command {
            Commands::Parse { config, name, .. } => {
                let cfg = Config::from_json(&config).unwrap();
                let effective = name.as_deref().or(cfg.name.as_deref()).unwrap();
                assert_eq!(effective, "cli_name");
            }
            _ => panic!("Expected Parse command"),
        }
    }

    #[test]
    fn test_name_priority_config_when_no_cli() {
        let cli = Cli::try_parse_from([
            "shclap",
            "parse",
            "--config",
            r#"{"name":"config_name"}"#,
            "--",
        ])
        .unwrap();

        match cli.command {
            Commands::Parse { config, name, .. } => {
                let cfg = Config::from_json(&config).unwrap();
                let effective = name.as_deref().or(cfg.name.as_deref()).unwrap();
                assert_eq!(effective, "config_name");
            }
            _ => panic!("Expected Parse command"),
        }
    }
}
