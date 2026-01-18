//! shclap - Clap-style argument parsing for shell scripts.

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use shclap::{generate_help, generate_output, generate_version, parse_args, Config};

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
    },

    /// Print version of the target script
    Version {
        /// JSON configuration for the target script
        #[arg(long)]
        config: String,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Parse {
            config,
            prefix,
            args,
        } => {
            let cfg = Config::from_json(&config).context("failed to parse config JSON")?;
            cfg.validate().context("invalid config")?;

            let effective_prefix = prefix.as_deref().unwrap_or_else(|| cfg.effective_prefix());

            let parsed = parse_args(&cfg, &args).context("failed to parse arguments")?;
            let path = generate_output(&parsed, effective_prefix)
                .context("failed to generate output file")?;

            println!("{}", path.display());
        }
        Commands::Help { config } => {
            let cfg = Config::from_json(&config).context("failed to parse config JSON")?;
            print!("{}", generate_help(&cfg));
        }
        Commands::Version { config } => {
            let cfg = Config::from_json(&config).context("failed to parse config JSON")?;
            print!("{}", generate_version(&cfg));
        }
    }

    Ok(())
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
                prefix,
                args,
            } => {
                assert_eq!(config, r#"{"name":"test"}"#);
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
            Commands::Parse { prefix, .. } => {
                assert_eq!(prefix, Some("MYAPP_".to_string()));
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
            Commands::Help { config } => {
                assert_eq!(config, r#"{"name":"test","description":"A test"}"#);
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
            Commands::Version { config } => {
                assert_eq!(config, r#"{"name":"test","version":"1.0.0"}"#);
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
}
