//! shclap - Clap-style argument parsing for shell scripts.

use anyhow::{Context, Result};
use clap::Parser;
use shclap::{generate_help, generate_output, generate_version, parse_args, Config};

/// Clap-style argument parsing for shell scripts.
#[derive(Parser, Debug)]
#[command(name = "shclap", version, about)]
struct Cli {
    /// JSON configuration for the target script
    #[arg(long)]
    config: String,

    /// Environment variable prefix (overrides config prefix)
    #[arg(long)]
    prefix: Option<String>,

    /// Arguments to parse for the target script
    #[arg(last = true)]
    script_args: Vec<String>,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    // Parse and validate config
    let config = Config::from_json(&cli.config).context("failed to parse config JSON")?;
    config.validate().context("invalid config")?;

    // Determine effective prefix: CLI > config > default
    let prefix = cli
        .prefix
        .as_deref()
        .unwrap_or_else(|| config.effective_prefix());

    // Check for --help or -h in script args
    if cli.script_args.iter().any(|a| a == "--help" || a == "-h") {
        print!("{}", generate_help(&config));
        return Ok(());
    }

    // Check for --version in script args
    if cli.script_args.iter().any(|a| a == "--version") {
        print!("{}", generate_version(&config));
        return Ok(());
    }

    // Parse the script arguments
    let parsed = parse_args(&config, &cli.script_args).context("failed to parse arguments")?;

    // Generate output file
    let output_path = generate_output(&parsed, prefix).context("failed to generate output file")?;

    // Print the path so the shell script can source it
    println!("{}", output_path.display());

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn test_cli_parses_config() {
        let cli = Cli::try_parse_from(["shclap", "--config", r#"{"name":"test"}"#, "--"]).unwrap();

        assert_eq!(cli.config, r#"{"name":"test"}"#);
        assert!(cli.prefix.is_none());
        assert!(cli.script_args.is_empty());
    }

    #[test]
    fn test_cli_parses_prefix() {
        let cli = Cli::try_parse_from([
            "shclap",
            "--config",
            r#"{"name":"test"}"#,
            "--prefix",
            "MYAPP_",
            "--",
        ])
        .unwrap();

        assert_eq!(cli.prefix, Some("MYAPP_".to_string()));
    }

    #[test]
    fn test_cli_parses_script_args() {
        let cli = Cli::try_parse_from([
            "shclap",
            "--config",
            r#"{"name":"test"}"#,
            "--",
            "-v",
            "--output",
            "file.txt",
            "input.txt",
        ])
        .unwrap();

        assert_eq!(
            cli.script_args,
            vec!["-v", "--output", "file.txt", "input.txt"]
        );
    }

    #[test]
    fn test_cli_requires_config() {
        let result = Cli::try_parse_from(["shclap", "--"]);
        assert!(result.is_err());
    }

    #[test]
    fn test_cli_help() {
        // Just verify the command can generate help without panicking
        Cli::command().debug_assert();
    }

    #[test]
    fn test_prefix_priority_cli_overrides_config() {
        // When CLI prefix is set, it should override config prefix
        let cli = Cli::try_parse_from([
            "shclap",
            "--config",
            r#"{"name":"test","prefix":"CONFIG_"}"#,
            "--prefix",
            "CLI_",
            "--",
        ])
        .unwrap();

        let config = Config::from_json(&cli.config).unwrap();

        // CLI prefix takes priority
        let effective = cli
            .prefix
            .as_deref()
            .unwrap_or_else(|| config.effective_prefix());

        assert_eq!(effective, "CLI_");
    }

    #[test]
    fn test_prefix_priority_config_when_no_cli() {
        let cli = Cli::try_parse_from([
            "shclap",
            "--config",
            r#"{"name":"test","prefix":"CONFIG_"}"#,
            "--",
        ])
        .unwrap();

        let config = Config::from_json(&cli.config).unwrap();

        let effective = cli
            .prefix
            .as_deref()
            .unwrap_or_else(|| config.effective_prefix());

        assert_eq!(effective, "CONFIG_");
    }

    #[test]
    fn test_prefix_priority_default_when_neither_set() {
        let cli = Cli::try_parse_from(["shclap", "--config", r#"{"name":"test"}"#, "--"]).unwrap();

        let config = Config::from_json(&cli.config).unwrap();

        let effective = cli
            .prefix
            .as_deref()
            .unwrap_or_else(|| config.effective_prefix());

        assert_eq!(effective, "SHCLAP_");
    }
}
