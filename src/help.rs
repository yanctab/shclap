//! Help and version text generation for target scripts using Clap.

use crate::command::build_command;
use crate::config::Config;

/// Generate the full help text for a script.
///
/// The `effective_name` parameter is the program name to use (from CLI --name or config name).
pub fn generate_help(config: &Config, effective_name: &str) -> String {
    let mut cmd = build_command(config, effective_name);
    cmd.render_help().to_string()
}

/// Generate version string.
///
/// The `effective_name` parameter is the program name to use (from CLI --name or config name).
pub fn generate_version(config: &Config, effective_name: &str) -> String {
    let mut version = effective_name.to_string();
    if let Some(ref v) = config.version {
        version.push(' ');
        version.push_str(v);
    }
    version.push('\n');
    version
}

#[cfg(test)]
#[allow(clippy::needless_update)]
mod tests {
    use super::*;
    // Only the tests construct configs by hand; the module itself needs Config.
    use crate::config::{ArgConfig, ArgType, ValueType};

    fn make_config(
        name: &str,
        description: Option<&str>,
        version: Option<&str>,
        args: Vec<ArgConfig>,
    ) -> Config {
        Config {
            schema_version: 1,
            name: Some(name.to_string()),
            description: description.map(|s| s.to_string()),
            version: version.map(|s| s.to_string()),
            prefix: None,
            args,
            subcommands: vec![],
            container: None,
            ..Default::default()
        }
    }

    /// Get the effective name from config, defaulting to "test".
    fn get_name(config: &Config) -> &str {
        config.name.as_deref().unwrap_or("test")
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
            env: None,
            multiple: false,
            num_args: None,
            delimiter: None,
            choices: None,
            value_type: ValueType::String,
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
            env: None,
            multiple: false,
            num_args: None,
            delimiter: None,
            choices: None,
            value_type: ValueType::String,
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
            env: None,
            multiple: false,
            num_args: None,
            delimiter: None,
            choices: None,
            value_type: ValueType::String,
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

        let help = generate_help(&config, get_name(&config));

        // Check essential content is present (Clap format may differ slightly)
        assert!(help.contains("myscript"), "Help should contain script name");
        assert!(
            help.contains("My awesome script"),
            "Help should contain description"
        );
        assert!(
            help.contains("-v") || help.contains("--verbose"),
            "Help should contain verbose flag"
        );
        assert!(
            help.contains("-o") || help.contains("--output"),
            "Help should contain output option"
        );
        assert!(
            help.contains("-h") || help.contains("--help"),
            "Help should contain help option"
        );
    }

    #[test]
    fn test_generate_help_minimal() {
        let config = make_config("minimal", None, None, vec![]);

        let help = generate_help(&config, get_name(&config));

        assert!(help.contains("minimal"), "Help should contain script name");
        assert!(
            help.contains("-h") || help.contains("--help"),
            "Help should contain help option"
        );
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

        let help = generate_help(&config, get_name(&config));

        assert!(help.contains("out.txt"), "Help should show default value");
    }

    #[test]
    fn test_generate_version() {
        let config = make_config("myapp", None, Some("2.1.0"), vec![]);
        let version = generate_version(&config, get_name(&config));
        assert_eq!(version, "myapp 2.1.0\n");

        let config_no_version = make_config("myapp", None, None, vec![]);
        let version = generate_version(&config_no_version, get_name(&config_no_version));
        assert_eq!(version, "myapp\n");
    }

    #[test]
    fn test_generate_help_with_name_override() {
        // Test that --name override works correctly
        let config = make_config("config_name", None, None, vec![]);

        let help = generate_help(&config, "override_name");

        assert!(
            help.contains("override_name"),
            "Help should contain the override name"
        );
        assert!(
            !help.contains("config_name"),
            "Help should not contain the config name"
        );
    }

    #[test]
    fn test_generate_help_with_long_fallback() {
        // Test that argument name is used as long option when neither short nor long specified
        let config = make_config(
            "test",
            None,
            None,
            vec![make_flag(
                "verbose",
                None,
                None,
                Some("Enable verbose mode"),
            )],
        );

        let help = generate_help(&config, get_name(&config));

        assert!(
            help.contains("--verbose"),
            "Help should show --verbose (name used as long fallback)"
        );
    }

    #[test]
    fn test_generate_help_with_choices() {
        // Test that choices are shown in help output
        let config = Config {
            schema_version: 2,
            name: Some("test".to_string()),
            description: None,
            version: None,
            prefix: None,
            args: vec![ArgConfig {
                name: "format".to_string(),
                short: Some('f'),
                long: Some("format".to_string()),
                arg_type: ArgType::Option,
                required: false,
                default: None,
                help: Some("Output format".to_string()),
                env: None,
                multiple: false,
                num_args: None,
                delimiter: None,
                choices: Some(vec![
                    "json".to_string(),
                    "yaml".to_string(),
                    "toml".to_string(),
                ]),
                value_type: ValueType::String,
            }],
            subcommands: vec![],
            container: None,
            ..Default::default()
        };

        let help = generate_help(&config, get_name(&config));

        // Clap shows possible values in help
        assert!(
            help.contains("json") && help.contains("yaml") && help.contains("toml"),
            "Help should show choices: {}",
            help
        );
    }

    #[test]
    fn test_generate_help_with_value_type_bool() {
        // Test that bool value_type shows true/false in help
        let config = Config {
            schema_version: 2,
            name: Some("test".to_string()),
            description: None,
            version: None,
            prefix: None,
            args: vec![ArgConfig {
                name: "enabled".to_string(),
                short: Some('e'),
                long: Some("enabled".to_string()),
                arg_type: ArgType::Option,
                required: false,
                default: None,
                help: Some("Enable feature".to_string()),
                env: None,
                multiple: false,
                num_args: None,
                delimiter: None,
                choices: None,
                value_type: ValueType::Bool,
            }],
            subcommands: vec![],
            container: None,
            ..Default::default()
        };

        let help = generate_help(&config, get_name(&config));

        // Clap shows possible values for bool
        assert!(
            help.contains("true") && help.contains("false"),
            "Help should show true/false for bool value_type: {}",
            help
        );
    }

    #[test]
    fn test_generate_help_with_value_type_double() {
        // Create config with double value_type
        let double_config = Config {
            schema_version: 2,
            name: Some("test".to_string()),
            description: None,
            version: None,
            prefix: None,
            args: vec![ArgConfig {
                name: "ratio".to_string(),
                short: Some('r'),
                long: Some("ratio".to_string()),
                arg_type: ArgType::Option,
                required: false,
                default: None,
                help: Some("Calculation ratio".to_string()),
                env: None,
                multiple: false,
                num_args: None,
                delimiter: None,
                choices: None,
                value_type: ValueType::Double,
            }],
            subcommands: vec![],
            container: None,
            ..Default::default()
        };

        let double_help = generate_help(&double_config, get_name(&double_config));

        // Help should be non-empty
        assert!(!double_help.is_empty(), "Help should not be empty");

        // Help should not contain true/false (no confusion with bool)
        assert!(
            !(double_help.contains("true") && double_help.contains("false")),
            "Help should not show true/false for double value_type: {}",
            double_help
        );
    }
}
