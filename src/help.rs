//! Help text generation for target scripts.

use crate::config::{ArgType, Config};

/// Generate the full help text for a script.
pub fn generate_help(config: &Config) -> String {
    let mut output = String::new();

    // Header: name and version
    output.push_str(&config.name);
    if let Some(ref version) = config.version {
        output.push_str(" v");
        output.push_str(version);
    }
    output.push('\n');

    // Description
    if let Some(ref desc) = config.description {
        output.push_str(desc);
        output.push('\n');
    }

    // Usage
    output.push('\n');
    output.push_str(&generate_usage(config));

    // Positional args section
    let positionals: Vec<_> = config
        .args
        .iter()
        .filter(|a| a.arg_type == ArgType::Positional)
        .collect();

    if !positionals.is_empty() {
        output.push_str("\nARGS:\n");
        for arg in positionals {
            output.push_str("    <");
            output.push_str(&arg.name.to_uppercase());
            output.push('>');

            if let Some(ref help) = arg.help {
                // Pad to column 16 minimum
                let name_len = arg.name.len() + 2; // < and >
                let padding = if name_len < 12 { 12 - name_len } else { 4 };
                for _ in 0..padding {
                    output.push(' ');
                }
                output.push_str(help);
            }
            output.push('\n');
        }
    }

    // Options section
    let options: Vec<_> = config
        .args
        .iter()
        .filter(|a| a.arg_type != ArgType::Positional)
        .collect();

    if !options.is_empty() {
        output.push_str("\nOPTIONS:\n");
        for arg in &options {
            output.push_str("    ");

            // Build the option string
            let mut opt_str = String::new();
            if let Some(short) = arg.short {
                opt_str.push('-');
                opt_str.push(short);
                if arg.long.is_some() {
                    opt_str.push_str(", ");
                }
            }
            if let Some(ref long) = arg.long {
                opt_str.push_str("--");
                opt_str.push_str(long);
            }

            // Add value placeholder for options
            if arg.arg_type == ArgType::Option {
                opt_str.push_str(" <VALUE>");
            }

            output.push_str(&opt_str);

            // Add help text
            if let Some(ref help) = arg.help {
                let padding = if opt_str.len() < 20 {
                    20 - opt_str.len()
                } else {
                    4
                };
                for _ in 0..padding {
                    output.push(' ');
                }
                output.push_str(help);
            }

            // Add required/default info
            if arg.required {
                output.push_str(" (required)");
            } else if let Some(ref default) = arg.default {
                output.push_str(" [default: ");
                output.push_str(default);
                output.push(']');
            }

            output.push('\n');
        }

        // Always add help option
        output.push_str("    -h, --help          Show this help message\n");
    }

    output
}

/// Generate just the usage line.
pub fn generate_usage(config: &Config) -> String {
    let mut usage = String::from("USAGE:\n    ");
    usage.push_str(&config.name);

    // Check if there are any options
    let has_options = config
        .args
        .iter()
        .any(|a| a.arg_type != ArgType::Positional);

    if has_options {
        usage.push_str(" [OPTIONS]");
    }

    // Add positional args
    for arg in &config.args {
        if arg.arg_type == ArgType::Positional {
            usage.push(' ');
            if arg.required {
                usage.push('<');
                usage.push_str(&arg.name.to_uppercase());
                usage.push('>');
            } else {
                usage.push('[');
                usage.push_str(&arg.name.to_uppercase());
                usage.push(']');
            }
        }
    }

    usage.push('\n');
    usage
}

/// Generate version string.
pub fn generate_version(config: &Config) -> String {
    let mut version = config.name.clone();
    if let Some(ref v) = config.version {
        version.push(' ');
        version.push_str(v);
    }
    version.push('\n');
    version
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ArgConfig;

    fn make_config(
        name: &str,
        description: Option<&str>,
        version: Option<&str>,
        args: Vec<ArgConfig>,
    ) -> Config {
        Config {
            name: name.to_string(),
            description: description.map(|s| s.to_string()),
            version: version.map(|s| s.to_string()),
            prefix: None,
            args,
        }
    }

    fn make_flag(
        name: &str,
        short: Option<char>,
        long: Option<&str>,
        help: Option<&str>,
    ) -> ArgConfig {
        ArgConfig {
            name: name.to_string(),
            short,
            long: long.map(|s| s.to_string()),
            arg_type: ArgType::Flag,
            required: false,
            default: None,
            help: help.map(|s| s.to_string()),
        }
    }

    fn make_option(
        name: &str,
        short: Option<char>,
        long: Option<&str>,
        required: bool,
        default: Option<&str>,
        help: Option<&str>,
    ) -> ArgConfig {
        ArgConfig {
            name: name.to_string(),
            short,
            long: long.map(|s| s.to_string()),
            arg_type: ArgType::Option,
            required,
            default: default.map(|s| s.to_string()),
            help: help.map(|s| s.to_string()),
        }
    }

    fn make_positional(name: &str, required: bool, help: Option<&str>) -> ArgConfig {
        ArgConfig {
            name: name.to_string(),
            short: None,
            long: None,
            arg_type: ArgType::Positional,
            required,
            default: None,
            help: help.map(|s| s.to_string()),
        }
    }

    #[test]
    fn test_generate_help_all_types() {
        let config = make_config(
            "myscript",
            Some("My awesome script"),
            Some("1.0.0"),
            vec![
                make_flag(
                    "verbose",
                    Some('v'),
                    Some("verbose"),
                    Some("Enable verbose output"),
                ),
                make_option(
                    "output",
                    Some('o'),
                    Some("output"),
                    true,
                    None,
                    Some("Output file"),
                ),
                make_positional("input", true, Some("Input file to process")),
            ],
        );

        let help = generate_help(&config);

        assert!(help.contains("myscript v1.0.0"));
        assert!(help.contains("My awesome script"));
        assert!(help.contains("USAGE:"));
        assert!(help.contains("myscript [OPTIONS] <INPUT>"));
        assert!(help.contains("ARGS:"));
        assert!(help.contains("<INPUT>"));
        assert!(help.contains("Input file to process"));
        assert!(help.contains("OPTIONS:"));
        assert!(help.contains("-v, --verbose"));
        assert!(help.contains("Enable verbose output"));
        assert!(help.contains("-o, --output <VALUE>"));
        assert!(help.contains("(required)"));
        assert!(help.contains("-h, --help"));
    }

    #[test]
    fn test_generate_help_minimal() {
        let config = make_config("minimal", None, None, vec![]);

        let help = generate_help(&config);

        assert!(help.contains("minimal\n"));
        assert!(help.contains("USAGE:\n    minimal\n"));
        assert!(!help.contains("ARGS:"));
        assert!(!help.contains("OPTIONS:"));
    }

    #[test]
    fn test_generate_help_with_defaults() {
        let config = make_config(
            "test",
            None,
            None,
            vec![make_option(
                "output",
                Some('o'),
                Some("output"),
                false,
                Some("out.txt"),
                Some("Output file"),
            )],
        );

        let help = generate_help(&config);

        assert!(help.contains("[default: out.txt]"));
    }

    #[test]
    fn test_generate_usage() {
        let config = make_config(
            "test",
            None,
            None,
            vec![
                make_flag("verbose", Some('v'), Some("verbose"), None),
                make_positional("input", true, None),
                make_positional("extra", false, None),
            ],
        );

        let usage = generate_usage(&config);

        assert_eq!(usage, "USAGE:\n    test [OPTIONS] <INPUT> [EXTRA]\n");
    }

    #[test]
    fn test_generate_version() {
        let config = make_config("myapp", None, Some("2.1.0"), vec![]);
        let version = generate_version(&config);
        assert_eq!(version, "myapp 2.1.0\n");

        let config_no_version = make_config("myapp", None, None, vec![]);
        let version = generate_version(&config_no_version);
        assert_eq!(version, "myapp\n");
    }

    #[test]
    fn test_optional_positional_formatting() {
        let config = make_config(
            "test",
            None,
            None,
            vec![make_positional("optional", false, Some("Optional input"))],
        );

        let usage = generate_usage(&config);
        assert!(usage.contains("[OPTIONAL]"));
    }
}
