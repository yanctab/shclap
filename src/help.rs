//! Help and version text generation for target scripts using Clap.

use crate::config::{ArgConfig, ArgType, Config, SubcommandConfig, ValueType};
use clap::{Arg, ArgAction, Command};

/// Build a Clap Command from a Config (for help/version generation).
fn build_command(config: &Config, effective_name: &str) -> Command {
    let mut cmd = Command::new(effective_name.to_string()).disable_help_subcommand(true);

    // Set version if provided
    if let Some(ref version) = config.version {
        cmd = cmd.version(version.clone());
    }

    // Set description if provided
    if let Some(ref description) = config.description {
        cmd = cmd.about(description.clone());
    }

    let prefix = config.effective_prefix();
    let schema_version = config.schema_version;

    // Track positional index for ordering
    let mut positional_index = 1usize;

    // Add arguments from config
    for arg_config in &config.args {
        let arg = build_arg(arg_config, &mut positional_index, prefix, schema_version);
        cmd = cmd.arg(arg);
    }

    // Add subcommands (schema v2)
    for subcmd_config in &config.subcommands {
        let subcmd = build_subcommand(subcmd_config, prefix, schema_version);
        cmd = cmd.subcommand(subcmd);
    }

    // Require subcommand if any defined
    if !config.subcommands.is_empty() {
        cmd = cmd.subcommand_required(true);
        cmd = cmd.arg_required_else_help(true);
    }

    cmd
}

/// Build a Clap Command for a subcommand config.
fn build_subcommand(config: &SubcommandConfig, prefix: &str, schema_version: u32) -> Command {
    let mut cmd = Command::new(config.name.clone());

    if let Some(ref help) = config.help {
        cmd = cmd.about(help.clone());
    }

    // Track positional index for ordering
    let mut positional_index = 1usize;

    // Add arguments
    for arg_config in &config.args {
        let arg = build_arg(arg_config, &mut positional_index, prefix, schema_version);
        cmd = cmd.arg(arg);
    }

    cmd
}

/// Build a Clap Arg from an ArgConfig.
fn build_arg(
    arg_config: &ArgConfig,
    positional_index: &mut usize,
    prefix: &str,
    schema_version: u32,
) -> Arg {
    let mut arg = Arg::new(arg_config.name.clone());

    match arg_config.arg_type {
        ArgType::Flag => {
            // For flags, use Count if multiple, SetTrue otherwise
            if arg_config.multiple {
                arg = arg.action(ArgAction::Count);
            } else {
                arg = arg.action(ArgAction::SetTrue);
            }

            if let Some(short) = arg_config.short {
                arg = arg.short(short);
            }

            // Use effective_long which falls back to name if neither short nor long is specified
            if let Some(long) = arg_config.effective_long() {
                arg = arg.long(long.to_string());
            }
        }
        ArgType::Option => {
            // For options, use Append if multiple, Set otherwise
            if arg_config.multiple {
                arg = arg.action(ArgAction::Append);
            } else {
                arg = arg.action(ArgAction::Set);
            }

            if let Some(short) = arg_config.short {
                arg = arg.short(short);
            }

            // Use effective_long which falls back to name if neither short nor long is specified
            if let Some(long) = arg_config.effective_long() {
                arg = arg.long(long.to_string());
            }

            arg = arg.value_name("VALUE");
        }
        ArgType::Positional => {
            arg = arg.index(*positional_index);
            *positional_index += 1;

            // For multiple positionals
            if arg_config.multiple {
                arg = arg.action(ArgAction::Append);
            }
        }
    }

    if arg_config.required {
        arg = arg.required(true);
    }

    if let Some(ref default) = arg_config.default {
        arg = arg.default_value(default.clone());
    }

    if let Some(ref help) = arg_config.help {
        arg = arg.help(help.clone());
    }

    // Schema v2: Environment variable fallback (auto-env or custom)
    if let Some(env_var) = arg_config.effective_env(prefix, schema_version) {
        arg = arg.env(env_var);
    }

    // Schema v2: num_args range
    if let Some(ref num_args) = arg_config.num_args {
        if let Some(range) = parse_num_args_range(num_args) {
            arg = arg.num_args(range);
        }
    }

    // Schema v2: Value delimiter
    if let Some(delim) = arg_config.delimiter {
        arg = arg.value_delimiter(delim);
    }

    // Schema v2: Choices (possible values) - takes precedence over value_type
    if let Some(ref choices) = arg_config.choices {
        arg = arg.value_parser(clap::builder::PossibleValuesParser::new(choices.clone()));
    } else {
        // Schema v2: Apply value_type parser if no choices specified
        match arg_config.value_type {
            ValueType::String => {} // Default, no special parser
            ValueType::Int => {
                arg = arg.value_parser(clap::value_parser!(i64));
            }
            ValueType::Bool => {
                arg = arg.value_parser(clap::builder::PossibleValuesParser::new(["true", "false"]));
            }
        }
    }

    arg
}

/// Parse a num_args string into a Clap ValueRange.
fn parse_num_args_range(s: &str) -> Option<clap::builder::ValueRange> {
    let s = s.trim();

    // Single number
    if let Ok(n) = s.parse::<usize>() {
        return Some(clap::builder::ValueRange::new(n..=n));
    }

    // Range formats
    if let Some(idx) = s.find("..") {
        let start: usize = s[..idx].parse().ok()?;
        let rest = &s[idx + 2..];

        if rest.is_empty() {
            // Unbounded: "N.."
            return Some(clap::builder::ValueRange::new(start..));
        }
        if let Ok(end) = rest.parse::<usize>() {
            // Exclusive: "N..M"
            return Some(clap::builder::ValueRange::new(start..end));
        }
        if let Some(stripped) = rest.strip_prefix('=') {
            if let Ok(end) = stripped.parse::<usize>() {
                // Inclusive: "N..=M"
                return Some(clap::builder::ValueRange::new(start..=end));
            }
        }
    }

    None
}

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
mod tests {
    use super::*;

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
        };

        let help = generate_help(&config, get_name(&config));

        // Clap shows possible values for bool
        assert!(
            help.contains("true") && help.contains("false"),
            "Help should show true/false for bool value_type: {}",
            help
        );
    }
}
