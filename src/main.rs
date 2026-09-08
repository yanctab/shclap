//! shclap - Clap-style argument parsing for shell scripts.

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use shclap::{
    detect_container_with, generate_container_reexec_output, generate_error_output, generate_help,
    generate_help_output, generate_output, generate_print, generate_version,
    generate_version_output, logging, parse_args, Config, ContainerSignal, ParseOutcome,
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

        /// Path to the calling script (pass "$0"); used for container re-exec volume mounts
        #[arg(long)]
        script: std::path::PathBuf,

        /// Filesystem root for container marker detection (test seam)
        #[arg(long, hide = true)]
        container_marker_root: Option<std::path::PathBuf>,

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

    /// Print how the script was called (reconstructs command line from env vars)
    Print {
        /// JSON configuration for the target script
        #[arg(long)]
        config: String,

        /// Application name (overrides config 'name' field)
        #[arg(long)]
        name: Option<String>,

        /// Environment variable prefix (overrides config)
        #[arg(long)]
        prefix: Option<String>,
    },

    /// Write a leveled log message to stderr
    Log {
        /// Log level (trace, debug, info, warn, error)
        level: String,

        /// Message parts to log
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        message: Vec<String>,
    },

    /// Collect files according to configuration
    Collect {
        /// JSON configuration (mutually exclusive with --config-file)
        #[arg(long, conflicts_with = "config_file")]
        config: Option<String>,

        /// Path to configuration file (mutually exclusive with --config)
        #[arg(long, conflicts_with = "config")]
        config_file: Option<std::path::PathBuf>,

        /// Output destination path
        #[arg(long)]
        out: String,

        /// Output type (dir or archive)
        #[arg(long, value_name = "TYPE")]
        r#type: shclap::collect::OutputType,

        /// Archive format (tar, tar.gz, zip) - only when type is archive
        #[arg(long)]
        archive_format: Option<shclap::collect::ArchiveFormat>,

        /// Bundle names to collect (can be specified multiple times)
        #[arg(long, value_name = "BUNDLE")]
        bundle: Vec<String>,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Parse {
            config,
            name,
            prefix,
            script,
            container_marker_root,
            args,
        } => {
            let (cfg, effective_name) = match load_config(&config, name.as_deref()) {
                Ok(loaded) => loaded,
                Err(e) => return output_error(&e),
            };

            let effective_prefix = prefix.as_deref().unwrap_or_else(|| cfg.effective_prefix());

            // Parse args first so that --help / --version outcomes bypass container dispatch.
            let parse_outcome = parse_args(&cfg, &args, &effective_name);

            // Container dispatch: only reexec on a real Success outcome when we are NOT already
            // inside a container.  Help, Version, and Error outcomes pass through unchanged.
            if matches!(parse_outcome, ParseOutcome::Success(_)) {
                if let Some(ref container) = cfg.container {
                    let marker_root = container_marker_root
                        .as_deref()
                        .unwrap_or(std::path::Path::new("/"));
                    let detected_signal =
                        detect_container_with(marker_root, |name| std::env::var(name).ok());

                    match detected_signal {
                        Some(ContainerSignal::ShclapInContainer) => {
                            // Already in a shclap-managed container — bypass silently.
                        }
                        Some(signal) => {
                            // Generic container signal — emit one diagnostic line and bypass.
                            eprintln!(
                                "shclap: container detected via {}, skipping reexec",
                                signal.signal_name()
                            );
                        }
                        None => {
                            // Not inside any container — reexec into the configured one.
                            let script_canonical = script.canonicalize().with_context(|| {
                                format!("failed to resolve script path: {}", script.display())
                            })?;
                            let path = generate_container_reexec_output(
                                container,
                                &cfg,
                                &script_canonical,
                            )
                            .context("failed to generate container reexec output file")?;
                            println!("{}", path.display());
                            return Ok(());
                        }
                    }
                }
            }

            // Handle parse result
            match parse_outcome {
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
            let (cfg, effective_name) =
                load_config(&config, name.as_deref()).map_err(|e| anyhow::anyhow!(e))?;

            print!("{}", generate_help(&cfg, &effective_name));
        }
        Commands::Version { config, name } => {
            let (cfg, effective_name) =
                load_config(&config, name.as_deref()).map_err(|e| anyhow::anyhow!(e))?;

            print!("{}", generate_version(&cfg, &effective_name));
        }
        Commands::Print {
            config,
            name,
            prefix,
        } => {
            let (cfg, effective_name) =
                load_config(&config, name.as_deref()).map_err(|e| anyhow::anyhow!(e))?;

            let effective_prefix = prefix.as_deref().unwrap_or_else(|| cfg.effective_prefix());

            println!(
                "{}",
                generate_print(&cfg, &effective_name, effective_prefix)
            );
        }
        Commands::Log { level, message } => {
            logging::run(&level, &message)?;
        }
        Commands::Collect {
            config,
            config_file,
            out,
            r#type,
            archive_format,
            bundle,
        } => {
            // Validate that exactly one of --config or --config-file is supplied
            let config_json = match (&config, &config_file) {
                (Some(_), Some(_)) => {
                    return output_error("--config and --config-file are mutually exclusive");
                }
                (None, None) => {
                    return output_error("either --config or --config-file must be supplied");
                }
                (Some(c), None) => c.clone(),
                (None, Some(f)) => {
                    std::fs::read_to_string(f).context("failed to read config file")?
                }
            };

            // Parse the config JSON
            let collect_config: shclap::collect::CollectConfig =
                serde_json::from_str(&config_json).context("failed to parse collect config")?;

            // Run the collection engine
            shclap::collect::run(collect_config, &out, r#type, archive_format, &bundle)?;
        }
    }

    Ok(())
}

/// Parse, expand, and validate a config, then resolve the effective app name.
///
/// Every subcommand that takes `--config` goes through here, so validation can
/// never be skipped for one of them. It used to be wired into `parse` only,
/// which left `help`, `version`, and `print` building a Clap command from an
/// unchecked config — a duplicate argument name tripped Clap's internal debug
/// assertion and aborted the process instead of reporting the config error.
///
/// The error is a plain `String` because callers render it two different ways:
/// `parse` writes it into a sourceable error file, the others bail through
/// `anyhow`.
fn load_config(config: &str, name: Option<&str>) -> Result<(Config, String), String> {
    // ConfigError::ParseError already says "failed to parse JSON config", so no
    // wrapper here — it used to produce that phrase twice in one message.
    let mut cfg = Config::from_json(config).map_err(|e| e.to_string())?;

    cfg.expand_vars(|name| std::env::var(name).ok())
        .map_err(|e| format!("environment variable expansion failed: {}", e))?;

    cfg.validate().map_err(|e| e.to_string())?;

    // CLI --name takes priority over the config 'name' field.
    let effective_name = match (name, cfg.name.as_deref()) {
        (Some(cli_name), _) => cli_name.to_string(),
        (None, Some(config_name)) => config_name.to_string(),
        (None, None) => {
            return Err("no application name provided: use --name or set 'name' in config".into())
        }
    };

    Ok((cfg, effective_name))
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
        let cli = Cli::try_parse_from([
            "shclap",
            "parse",
            "--config",
            r#"{"name":"test"}"#,
            "--script",
            "/test/script.sh",
            "--",
        ])
        .unwrap();

        match cli.command {
            Commands::Parse {
                config,
                name,
                prefix,
                script,
                container_marker_root,
                args,
            } => {
                assert_eq!(config, r#"{"name":"test"}"#);
                assert!(name.is_none());
                assert!(prefix.is_none());
                assert_eq!(script, std::path::PathBuf::from("/test/script.sh"));
                assert!(container_marker_root.is_none());
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
            "--script",
            "/test/script.sh",
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
            "shclap",
            "parse",
            "--config",
            r#"{}"#,
            "--script",
            "/test/script.sh",
            "--name",
            "myapp",
            "--",
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
            "--script",
            "/test/script.sh",
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
            "--script",
            "/test/script.sh",
            "--prefix",
            "CLI_",
            "--",
        ])
        .unwrap();

        match cli.command {
            Commands::Parse { config, prefix, .. } => {
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
            "--script",
            "/test/script.sh",
            "--",
        ])
        .unwrap();

        match cli.command {
            Commands::Parse { config, prefix, .. } => {
                let cfg = Config::from_json(&config).unwrap();
                let effective = prefix.as_deref().unwrap_or_else(|| cfg.effective_prefix());
                assert_eq!(effective, "CONFIG_");
            }
            _ => panic!("Expected Parse command"),
        }
    }

    #[test]
    fn test_prefix_priority_default_when_neither_set() {
        let cli = Cli::try_parse_from([
            "shclap",
            "parse",
            "--config",
            r#"{"name":"test"}"#,
            "--script",
            "/test/script.sh",
            "--",
        ])
        .unwrap();

        match cli.command {
            Commands::Parse { config, prefix, .. } => {
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
            "--script",
            "/test/script.sh",
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
            "--script",
            "/test/script.sh",
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

    #[test]
    fn test_parse_subcommand_parses_container_marker_root() {
        let cli = Cli::try_parse_from([
            "shclap",
            "parse",
            "--config",
            r#"{"name":"test"}"#,
            "--script",
            "/test/script.sh",
            "--container-marker-root",
            "/tmp/test",
            "--",
        ])
        .unwrap();

        match cli.command {
            Commands::Parse {
                container_marker_root,
                ..
            } => {
                assert_eq!(
                    container_marker_root,
                    Some(std::path::PathBuf::from("/tmp/test"))
                );
            }
            _ => panic!("Expected Parse command"),
        }
    }

    #[test]
    fn test_load_config_validates() {
        // Duplicate names used to reach Clap unchecked and abort the process
        // via its internal debug assertion; now they surface as a config error.
        let err = load_config(
            r#"{"name":"t","args":[{"name":"a","type":"flag"},{"name":"a","type":"flag"}]}"#,
            None,
        )
        .unwrap_err();
        assert_eq!(err, "duplicate argument name: a");
    }

    #[test]
    fn test_load_config_rejects_unsupported_schema_version() {
        let err = load_config(r#"{"schema_version":99,"name":"t"}"#, None).unwrap_err();
        assert!(
            err.contains("unsupported schema version 99"),
            "got: {}",
            err
        );
    }

    #[test]
    fn test_load_config_reports_parse_failure_once() {
        let err = load_config("not valid json", None).unwrap_err();
        assert!(
            err.starts_with("failed to parse JSON config: "),
            "got: {}",
            err
        );
        // The phrase used to appear twice: once from ConfigError, once from a wrapper.
        assert_eq!(err.matches("failed to parse JSON config").count(), 1);
    }

    #[test]
    fn test_load_config_requires_a_name() {
        let err = load_config(r#"{}"#, None).unwrap_err();
        assert!(err.contains("no application name provided"), "got: {}", err);
    }

    #[test]
    fn test_load_config_name_priority() {
        let (_, name) = load_config(r#"{"name":"from_config"}"#, Some("from_cli")).unwrap();
        assert_eq!(name, "from_cli");

        let (_, name) = load_config(r#"{"name":"from_config"}"#, None).unwrap();
        assert_eq!(name, "from_config");
    }

    #[test]
    fn test_load_config_expands_variables() {
        // Proves expand_vars is wired in without mutating the process
        // environment, which would race the other tests in this binary.
        let err = load_config(
            r#"{"name":"t","version":"v${SHCLAP_NO_SUCH_VAR_FOR_TESTS}"}"#,
            None,
        )
        .unwrap_err();
        assert!(
            err.contains("environment variable expansion failed"),
            "got: {}",
            err
        );
        assert!(err.contains("SHCLAP_NO_SUCH_VAR_FOR_TESTS"), "got: {}", err);
    }

    #[test]
    fn test_log_subcommand_parses_level_and_message() {
        let cli = Cli::try_parse_from(["shclap", "log", "info", "hello", "world"]).unwrap();

        match cli.command {
            Commands::Log { level, message } => {
                assert_eq!(level, "info");
                assert_eq!(message, vec!["hello", "world"]);
            }
            _ => panic!("Expected Log command"),
        }
    }
}
