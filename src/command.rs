//! Building a Clap `Command` from a shclap `Config`.
//!
//! Both parsing and help generation need the same command: `parser` runs it
//! against real arguments, `help` only renders it. They used to hold separate
//! copies of this builder that agreed by coincidence, so any schema addition
//! had to be made twice or help output would silently drift from the behaviour
//! it documents. There is one builder now.
//!
//! The copies differed in only two ways, neither of which justified a second
//! implementation: `help` omitted `allow_hyphen_values`, which affects value
//! parsing and not rendering, and `parser` set `disable_version_flag(false)`
//! and `disable_help_flag(false)`, which are already Clap's defaults.

use crate::config::{ArgConfig, ArgType, Config, SubcommandConfig, ValueType};
use clap::{Arg, ArgAction, Command};

/// Build the Clap command described by `config`, named `effective_name`.
pub fn build_command(config: &Config, effective_name: &str) -> Command {
    // disable_version_flag(false) and disable_help_flag(false) were set
    // explicitly here; both are Clap's defaults, so they are simply omitted.
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

            // Add short option
            if let Some(short) = arg_config.short {
                arg = arg.short(short);
            }

            // Add long option (with fallback to name if neither short nor long specified)
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

            // Add short option
            if let Some(short) = arg_config.short {
                arg = arg.short(short);
            }

            // Add long option (with fallback to name if neither short nor long specified)
            if let Some(long) = arg_config.effective_long() {
                arg = arg.long(long.to_string());
            }

            // Set value name for help display
            arg = arg.value_name("VALUE");

            // Allow attached values like -ofile.txt
            arg = arg.allow_hyphen_values(true);
        }
        ArgType::Positional => {
            arg = arg.index(*positional_index);
            *positional_index += 1;

            // Allow values that look like flags (e.g., after --)
            arg = arg.allow_hyphen_values(true);

            // For multiple positionals
            if arg_config.multiple {
                arg = arg.action(ArgAction::Append);
            }
        }
    }

    // Set required status
    if arg_config.required {
        arg = arg.required(true);
    }

    // Set default value
    if let Some(ref default) = arg_config.default {
        arg = arg.default_value(default.clone());
    }

    // Set help text
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
            ValueType::Double => {
                arg = arg.value_parser(clap::value_parser!(f64));
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_num_args_formats() {
        // Single number
        assert!(parse_num_args_range("3").is_some());
        // Unbounded
        assert!(parse_num_args_range("1..").is_some());
        // Exclusive range
        assert!(parse_num_args_range("2..5").is_some());
        // Inclusive range
        assert!(parse_num_args_range("1..=3").is_some());
        // Invalid
        assert!(parse_num_args_range("abc").is_none());
        // Surrounding whitespace is tolerated
        assert!(parse_num_args_range("  2..5  ").is_some());
    }

    #[test]
    fn test_build_command_uses_effective_name() {
        let config = Config {
            name: Some("from_config".to_string()),
            ..Default::default()
        };
        assert_eq!(build_command(&config, "chosen").get_name(), "chosen");
    }

    #[test]
    fn test_build_command_is_valid_for_clap() {
        // Catches the assertion failures Clap raises for a malformed command.
        let config = Config {
            schema_version: 2,
            name: Some("t".to_string()),
            description: Some("d".to_string()),
            version: Some("1.0".to_string()),
            ..Default::default()
        };
        build_command(&config, "t").debug_assert();
    }
}
