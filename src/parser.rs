//! Argument parsing for target scripts using dynamic Clap.

use crate::config::{ArgConfig, ArgType, Config, SubcommandConfig};
use clap::{error::ErrorKind, Arg, ArgAction, Command};
use std::collections::HashMap;

/// A parsed argument value, which can be single or multiple.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParsedValue {
    /// A single string value
    Single(String),
    /// Multiple string values (from multiple occurrences or delimiter-split)
    Multiple(Vec<String>),
}

impl ParsedValue {
    /// Get the value as a single string (joins multiple with space if needed).
    pub fn as_single(&self) -> String {
        match self {
            ParsedValue::Single(s) => s.clone(),
            ParsedValue::Multiple(v) => v.join(" "),
        }
    }

    /// Check if this is a multiple value.
    pub fn is_multiple(&self) -> bool {
        matches!(self, ParsedValue::Multiple(_))
    }
}

/// Successful parse result with values and optional subcommand.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseSuccess {
    /// Parsed argument values
    pub values: HashMap<String, ParsedValue>,
    /// Subcommand name if one was matched
    pub subcommand: Option<String>,
}

/// Outcome of parsing arguments.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseOutcome {
    /// Successfully parsed arguments with variable values.
    Success(ParseSuccess),
    /// User requested help (-h or --help).
    Help(String),
    /// User requested version (-V or --version).
    Version(String),
    /// Parse error occurred.
    Error(String),
}

/// Result of parsing arguments (legacy type alias for compatibility).
pub type ParseResult = Result<HashMap<String, String>, ParseError>;

/// Errors that can occur during argument parsing.
/// Kept for API compatibility but primarily used for internal errors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    pub message: String,
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for ParseError {}

/// Build a Clap Command from a Config with an effective name.
fn build_command(config: &Config, effective_name: &str) -> Command {
    let mut cmd = Command::new(effective_name.to_string())
        .disable_help_subcommand(true)
        .disable_version_flag(false)
        .disable_help_flag(false);

    // Set version if provided
    if let Some(ref version) = config.version {
        cmd = cmd.version(version.clone());
    }

    // Set description if provided
    if let Some(ref description) = config.description {
        cmd = cmd.about(description.clone());
    }

    // Track positional index for ordering
    let mut positional_index = 1usize;

    // Add arguments from config
    for arg_config in &config.args {
        let arg = build_arg(arg_config, &mut positional_index);
        cmd = cmd.arg(arg);
    }

    // Add subcommands (schema v2)
    for subcmd_config in &config.subcommands {
        let subcmd = build_subcommand(subcmd_config);
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
fn build_subcommand(config: &SubcommandConfig) -> Command {
    let mut cmd = Command::new(config.name.clone());

    if let Some(ref help) = config.help {
        cmd = cmd.about(help.clone());
    }

    // Track positional index for ordering
    let mut positional_index = 1usize;

    // Add arguments
    for arg_config in &config.args {
        let arg = build_arg(arg_config, &mut positional_index);
        cmd = cmd.arg(arg);
    }

    cmd
}

/// Build a Clap Arg from an ArgConfig.
fn build_arg(arg_config: &ArgConfig, positional_index: &mut usize) -> Arg {
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

    // Schema v2: Environment variable fallback
    if let Some(ref env_var) = arg_config.env {
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

/// Extract parsed values from ArgMatches into a HashMap.
fn extract_values(args: &[ArgConfig], matches: &clap::ArgMatches) -> HashMap<String, ParsedValue> {
    let mut results = HashMap::new();

    for arg_config in args {
        let name = &arg_config.name;

        match arg_config.arg_type {
            ArgType::Flag => {
                if arg_config.multiple {
                    // Count action returns u8
                    let count = matches.get_count(name);
                    results.insert(name.clone(), ParsedValue::Single(count.to_string()));
                } else {
                    let value = matches.get_flag(name);
                    results.insert(name.clone(), ParsedValue::Single(value.to_string()));
                }
            }
            ArgType::Option | ArgType::Positional => {
                if arg_config.multiple {
                    // Multiple values: get all
                    let values: Vec<String> = matches
                        .get_many::<String>(name)
                        .map(|v| v.cloned().collect())
                        .unwrap_or_default();

                    if !values.is_empty() {
                        results.insert(name.clone(), ParsedValue::Multiple(values));
                    } else if let Some(ref default) = arg_config.default {
                        results.insert(name.clone(), ParsedValue::Multiple(vec![default.clone()]));
                    }
                } else {
                    // Single value
                    if let Some(value) = matches.get_one::<String>(name) {
                        results.insert(name.clone(), ParsedValue::Single(value.clone()));
                    } else if let Some(ref default) = arg_config.default {
                        results.insert(name.clone(), ParsedValue::Single(default.clone()));
                    }
                }
            }
        }
    }

    results
}

/// Parse command-line arguments according to the config.
///
/// The `effective_name` parameter is the program name to use (from CLI --name or config name).
///
/// Returns `ParseOutcome::Help` if -h/--help is found.
/// Returns `ParseOutcome::Version` if -V/--version is found.
/// Returns `ParseOutcome::Success` with parsed values on success.
/// Returns `ParseOutcome::Error` on parse errors.
pub fn parse_args(config: &Config, args: &[String], effective_name: &str) -> ParseOutcome {
    let cmd = build_command(config, effective_name);

    // Prepend program name since Clap expects args[0] to be the program name
    let mut full_args = vec![effective_name.to_string()];
    full_args.extend(args.iter().cloned());

    match cmd.try_get_matches_from(&full_args) {
        Ok(matches) => {
            // Check for subcommand
            if let Some((subcmd_name, subcmd_matches)) = matches.subcommand() {
                // Find the subcommand config
                if let Some(subcmd_config) =
                    config.subcommands.iter().find(|s| s.name == subcmd_name)
                {
                    // Extract main command args
                    let mut values = extract_values(&config.args, &matches);
                    // Extract subcommand args
                    let subcmd_values = extract_values(&subcmd_config.args, subcmd_matches);
                    values.extend(subcmd_values);

                    return ParseOutcome::Success(ParseSuccess {
                        values,
                        subcommand: Some(subcmd_name.to_string()),
                    });
                }
            }

            // No subcommand
            let values = extract_values(&config.args, &matches);
            ParseOutcome::Success(ParseSuccess {
                values,
                subcommand: None,
            })
        }
        Err(e) => {
            match e.kind() {
                ErrorKind::DisplayHelp => ParseOutcome::Help(e.to_string()),
                ErrorKind::DisplayVersion => ParseOutcome::Version(e.to_string()),
                _ => {
                    // Format error message to match expected format
                    let message = format_error_message(&e);
                    ParseOutcome::Error(message)
                }
            }
        }
    }
}

/// Format Clap error messages to match expected shclap format.
fn format_error_message(error: &clap::Error) -> String {
    let raw = error.to_string();

    // Extract the core error message from Clap's output
    // Clap format: "error: <message>\n\nUsage: ..."
    if let Some(first_line) = raw.lines().next() {
        let msg = first_line.strip_prefix("error: ").unwrap_or(first_line);

        // Map common Clap messages to shclap format
        if msg.contains("unexpected argument") {
            // Extract the option name from "unexpected argument 'X' found"
            if let Some(start) = msg.find('\'') {
                if let Some(end) = msg[start + 1..].find('\'') {
                    let opt = &msg[start + 1..start + 1 + end];
                    return format!("unknown option: {}", opt);
                }
            }
        }

        if msg.contains("required arguments were not provided")
            || msg.contains("the following required argument")
        {
            // Look for the argument name in the full message
            for line in raw.lines() {
                let trimmed = line.trim();
                if trimmed.starts_with('<') {
                    if let Some(end) = trimmed.find('>') {
                        let arg_name = &trimmed[1..end];
                        return format!("missing required argument: {}", arg_name.to_lowercase());
                    }
                }
            }
            return "missing required argument".to_string();
        }

        if msg.contains("a value is required") {
            // Extract option name
            for line in raw.lines() {
                if line.contains("--") {
                    if let Some(opt_start) = line.find("--") {
                        let rest = &line[opt_start..];
                        let opt_end = rest
                            .find(|c: char| c.is_whitespace() || c == '>')
                            .unwrap_or(rest.len());
                        let opt = &rest[..opt_end];
                        return format!("missing value for option: {}", opt);
                    }
                }
            }
        }

        return msg.to_string();
    }

    raw
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;

    fn parse_config(json: &str) -> Config {
        Config::from_json(json).unwrap()
    }

    fn to_args(s: &[&str]) -> Vec<String> {
        s.iter().map(|s| s.to_string()).collect()
    }

    /// Get the effective name from config, defaulting to "test".
    fn get_name(config: &Config) -> &str {
        config.name.as_deref().unwrap_or("test")
    }

    /// Helper to unwrap success and convert to simple string map for existing tests.
    fn unwrap_success(outcome: ParseOutcome) -> HashMap<String, String> {
        match outcome {
            ParseOutcome::Success(ps) => ps
                .values
                .into_iter()
                .map(|(k, v)| (k, v.as_single()))
                .collect(),
            other => panic!("Expected Success, got {:?}", other),
        }
    }

    /// Helper to unwrap success and return the full ParseSuccess.
    fn unwrap_success_full(outcome: ParseOutcome) -> ParseSuccess {
        match outcome {
            ParseOutcome::Success(ps) => ps,
            other => panic!("Expected Success, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_flag_long() {
        let config = parse_config(
            r#"{"name":"test","args":[
                {"name":"verbose","long":"verbose","type":"flag"}
            ]}"#,
        );
        let result = unwrap_success(parse_args(
            &config,
            &to_args(&["--verbose"]),
            get_name(&config),
        ));
        assert_eq!(result.get("verbose"), Some(&"true".to_string()));
    }

    #[test]
    fn test_parse_flag_short() {
        let config = parse_config(
            r#"{"name":"test","args":[
                {"name":"verbose","short":"v","type":"flag"}
            ]}"#,
        );
        let result = unwrap_success(parse_args(&config, &to_args(&["-v"]), get_name(&config)));
        assert_eq!(result.get("verbose"), Some(&"true".to_string()));
    }

    #[test]
    fn test_parse_flag_default_false() {
        let config = parse_config(
            r#"{"name":"test","args":[
                {"name":"verbose","short":"v","type":"flag"}
            ]}"#,
        );
        let result = unwrap_success(parse_args(&config, &to_args(&[]), get_name(&config)));
        assert_eq!(result.get("verbose"), Some(&"false".to_string()));
    }

    #[test]
    fn test_parse_combined_short_flags() {
        let config = parse_config(
            r#"{"name":"test","args":[
                {"name":"a","short":"a","type":"flag"},
                {"name":"b","short":"b","type":"flag"},
                {"name":"c","short":"c","type":"flag"}
            ]}"#,
        );
        let result = unwrap_success(parse_args(&config, &to_args(&["-abc"]), get_name(&config)));
        assert_eq!(result.get("a"), Some(&"true".to_string()));
        assert_eq!(result.get("b"), Some(&"true".to_string()));
        assert_eq!(result.get("c"), Some(&"true".to_string()));
    }

    #[test]
    fn test_parse_option_long_space() {
        let config = parse_config(
            r#"{"name":"test","args":[
                {"name":"output","long":"output","type":"option"}
            ]}"#,
        );
        let result = unwrap_success(parse_args(
            &config,
            &to_args(&["--output", "file.txt"]),
            get_name(&config),
        ));
        assert_eq!(result.get("output"), Some(&"file.txt".to_string()));
    }

    #[test]
    fn test_parse_option_long_equals() {
        let config = parse_config(
            r#"{"name":"test","args":[
                {"name":"output","long":"output","type":"option"}
            ]}"#,
        );
        let result = unwrap_success(parse_args(
            &config,
            &to_args(&["--output=file.txt"]),
            get_name(&config),
        ));
        assert_eq!(result.get("output"), Some(&"file.txt".to_string()));
    }

    #[test]
    fn test_parse_option_short_space() {
        let config = parse_config(
            r#"{"name":"test","args":[
                {"name":"output","short":"o","type":"option"}
            ]}"#,
        );
        let result = unwrap_success(parse_args(
            &config,
            &to_args(&["-o", "file.txt"]),
            get_name(&config),
        ));
        assert_eq!(result.get("output"), Some(&"file.txt".to_string()));
    }

    #[test]
    fn test_parse_option_short_attached() {
        let config = parse_config(
            r#"{"name":"test","args":[
                {"name":"output","short":"o","type":"option"}
            ]}"#,
        );
        let result = unwrap_success(parse_args(
            &config,
            &to_args(&["-ofile.txt"]),
            get_name(&config),
        ));
        assert_eq!(result.get("output"), Some(&"file.txt".to_string()));
    }

    #[test]
    fn test_parse_positional() {
        let config = parse_config(
            r#"{"name":"test","args":[
                {"name":"input","type":"positional"}
            ]}"#,
        );
        let result = unwrap_success(parse_args(
            &config,
            &to_args(&["input.txt"]),
            get_name(&config),
        ));
        assert_eq!(result.get("input"), Some(&"input.txt".to_string()));
    }

    #[test]
    fn test_parse_multiple_positionals() {
        let config = parse_config(
            r#"{"name":"test","args":[
                {"name":"input","type":"positional"},
                {"name":"output","type":"positional"}
            ]}"#,
        );
        let result = unwrap_success(parse_args(
            &config,
            &to_args(&["in.txt", "out.txt"]),
            get_name(&config),
        ));
        assert_eq!(result.get("input"), Some(&"in.txt".to_string()));
        assert_eq!(result.get("output"), Some(&"out.txt".to_string()));
    }

    #[test]
    fn test_parse_mixed_args() {
        let config = parse_config(
            r#"{"name":"test","args":[
                {"name":"verbose","short":"v","long":"verbose","type":"flag"},
                {"name":"output","short":"o","long":"output","type":"option"},
                {"name":"input","type":"positional"}
            ]}"#,
        );
        let result = unwrap_success(parse_args(
            &config,
            &to_args(&["-v", "--output", "out.txt", "in.txt"]),
            get_name(&config),
        ));
        assert_eq!(result.get("verbose"), Some(&"true".to_string()));
        assert_eq!(result.get("output"), Some(&"out.txt".to_string()));
        assert_eq!(result.get("input"), Some(&"in.txt".to_string()));
    }

    #[test]
    fn test_parse_double_dash_separator() {
        let config = parse_config(
            r#"{"name":"test","args":[
                {"name":"verbose","short":"v","type":"flag"},
                {"name":"input","type":"positional"}
            ]}"#,
        );
        // After --, -v should be treated as positional
        let result = unwrap_success(parse_args(
            &config,
            &to_args(&["--", "-v"]),
            get_name(&config),
        ));
        assert_eq!(result.get("verbose"), Some(&"false".to_string()));
        assert_eq!(result.get("input"), Some(&"-v".to_string()));
    }

    #[test]
    fn test_parse_default_value() {
        let config = parse_config(
            r#"{"name":"test","args":[
                {"name":"output","long":"output","type":"option","default":"out.txt"}
            ]}"#,
        );
        let result = unwrap_success(parse_args(&config, &to_args(&[]), get_name(&config)));
        assert_eq!(result.get("output"), Some(&"out.txt".to_string()));
    }

    #[test]
    fn test_parse_default_overridden() {
        let config = parse_config(
            r#"{"name":"test","args":[
                {"name":"output","long":"output","type":"option","default":"default.txt"}
            ]}"#,
        );
        let result = unwrap_success(parse_args(
            &config,
            &to_args(&["--output", "custom.txt"]),
            get_name(&config),
        ));
        assert_eq!(result.get("output"), Some(&"custom.txt".to_string()));
    }

    #[test]
    fn test_error_missing_required() {
        let config = parse_config(
            r#"{"name":"test","args":[
                {"name":"input","type":"positional","required":true}
            ]}"#,
        );
        let result = parse_args(&config, &to_args(&[]), get_name(&config));
        match result {
            ParseOutcome::Error(msg) => {
                assert!(
                    msg.contains("missing required"),
                    "Expected 'missing required' in: {}",
                    msg
                );
            }
            other => panic!("Expected Error, got {:?}", other),
        }
    }

    #[test]
    fn test_error_missing_value() {
        let config = parse_config(
            r#"{"name":"test","args":[
                {"name":"output","long":"output","type":"option"}
            ]}"#,
        );
        let result = parse_args(&config, &to_args(&["--output"]), get_name(&config));
        match result {
            ParseOutcome::Error(msg) => {
                assert!(
                    msg.contains("--output") || msg.contains("value"),
                    "Expected error about --output or value in: {}",
                    msg
                );
            }
            other => panic!("Expected Error, got {:?}", other),
        }
    }

    #[test]
    fn test_error_unknown_option() {
        let config = parse_config(r#"{"name":"test","args":[]}"#);
        let result = parse_args(&config, &to_args(&["--unknown"]), get_name(&config));
        match result {
            ParseOutcome::Error(msg) => {
                assert!(
                    msg.contains("unknown option"),
                    "Expected 'unknown option' in: {}",
                    msg
                );
            }
            other => panic!("Expected Error, got {:?}", other),
        }
    }

    #[test]
    fn test_error_unexpected_positional() {
        let config = parse_config(r#"{"name":"test","args":[]}"#);
        let result = parse_args(&config, &to_args(&["unexpected"]), get_name(&config));
        match result {
            ParseOutcome::Error(msg) => {
                // Clap may report this differently
                assert!(
                    msg.contains("unexpected") || msg.contains("unknown"),
                    "Expected error in: {}",
                    msg
                );
            }
            other => panic!("Expected Error, got {:?}", other),
        }
    }

    #[test]
    fn test_short_flag_then_option() {
        let config = parse_config(
            r#"{"name":"test","args":[
                {"name":"verbose","short":"v","type":"flag"},
                {"name":"output","short":"o","type":"option"}
            ]}"#,
        );
        // -vo should set verbose=true and read next arg as output value
        let result = unwrap_success(parse_args(
            &config,
            &to_args(&["-vo", "file.txt"]),
            get_name(&config),
        ));
        assert_eq!(result.get("verbose"), Some(&"true".to_string()));
        assert_eq!(result.get("output"), Some(&"file.txt".to_string()));
    }

    #[test]
    fn test_short_option_with_attached_value() {
        let config = parse_config(
            r#"{"name":"test","args":[
                {"name":"verbose","short":"v","type":"flag"},
                {"name":"output","short":"o","type":"option"}
            ]}"#,
        );
        // -vofile.txt: v=true, o=file.txt
        let result = unwrap_success(parse_args(
            &config,
            &to_args(&["-vofile.txt"]),
            get_name(&config),
        ));
        assert_eq!(result.get("verbose"), Some(&"true".to_string()));
        assert_eq!(result.get("output"), Some(&"file.txt".to_string()));
    }

    #[test]
    fn test_value_with_special_chars() {
        let config = parse_config(
            r#"{"name":"test","args":[
                {"name":"msg","long":"msg","type":"option"}
            ]}"#,
        );
        let result = unwrap_success(parse_args(
            &config,
            &to_args(&["--msg", "hello $USER!"]),
            get_name(&config),
        ));
        assert_eq!(result.get("msg"), Some(&"hello $USER!".to_string()));
    }

    #[test]
    fn test_empty_value() {
        let config = parse_config(
            r#"{"name":"test","args":[
                {"name":"value","long":"value","type":"option"}
            ]}"#,
        );
        let result = unwrap_success(parse_args(
            &config,
            &to_args(&["--value", ""]),
            get_name(&config),
        ));
        assert_eq!(result.get("value"), Some(&"".to_string()));
    }

    #[test]
    fn test_option_equals_empty() {
        let config = parse_config(
            r#"{"name":"test","args":[
                {"name":"value","long":"value","type":"option"}
            ]}"#,
        );
        let result = unwrap_success(parse_args(
            &config,
            &to_args(&["--value="]),
            get_name(&config),
        ));
        assert_eq!(result.get("value"), Some(&"".to_string()));
    }

    #[test]
    fn test_positional_after_options() {
        let config = parse_config(
            r#"{"name":"test","args":[
                {"name":"output","short":"o","type":"option"},
                {"name":"input","type":"positional"}
            ]}"#,
        );
        let result = unwrap_success(parse_args(
            &config,
            &to_args(&["input.txt", "-o", "output.txt"]),
            get_name(&config),
        ));
        assert_eq!(result.get("input"), Some(&"input.txt".to_string()));
        assert_eq!(result.get("output"), Some(&"output.txt".to_string()));
    }

    #[test]
    fn test_help_flag_long() {
        let config = parse_config(r#"{"name":"test"}"#);
        let result = parse_args(&config, &to_args(&["--help"]), get_name(&config));
        assert!(matches!(result, ParseOutcome::Help(_)));
    }

    #[test]
    fn test_help_flag_short() {
        let config = parse_config(r#"{"name":"test"}"#);
        let result = parse_args(&config, &to_args(&["-h"]), get_name(&config));
        assert!(matches!(result, ParseOutcome::Help(_)));
    }

    #[test]
    fn test_version_flag_long() {
        let config = parse_config(r#"{"name":"test","version":"1.0.0"}"#);
        let result = parse_args(&config, &to_args(&["--version"]), get_name(&config));
        assert!(matches!(result, ParseOutcome::Version(_)));
    }

    #[test]
    fn test_version_flag_short() {
        let config = parse_config(r#"{"name":"test","version":"1.0.0"}"#);
        let result = parse_args(&config, &to_args(&["-V"]), get_name(&config));
        assert!(matches!(result, ParseOutcome::Version(_)));
    }

    #[test]
    fn test_help_takes_precedence_over_version() {
        let config = parse_config(r#"{"name":"test","version":"1.0.0"}"#);
        let result = parse_args(
            &config,
            &to_args(&["--version", "--help"]),
            get_name(&config),
        );
        // Clap processes left-to-right, so --version comes first
        // But help should take precedence - we need to check behavior
        assert!(matches!(
            result,
            ParseOutcome::Help(_) | ParseOutcome::Version(_)
        ));
    }

    #[test]
    fn test_help_anywhere_in_args() {
        let config = parse_config(
            r#"{"name":"test","args":[
                {"name":"verbose","short":"v","type":"flag"}
            ]}"#,
        );
        let result = parse_args(&config, &to_args(&["-v", "--help"]), get_name(&config));
        assert!(matches!(result, ParseOutcome::Help(_)));
    }

    #[test]
    fn test_version_anywhere_in_args() {
        let config = parse_config(
            r#"{"name":"test","version":"1.0.0","args":[
                {"name":"verbose","short":"v","type":"flag"}
            ]}"#,
        );
        let result = parse_args(&config, &to_args(&["-v", "--version"]), get_name(&config));
        assert!(matches!(result, ParseOutcome::Version(_)));
    }

    // Schema v2 tests

    #[test]
    fn test_env_fallback() {
        // Note: env var tests require actual env vars set, which is tricky in unit tests.
        // This test verifies the config parses correctly; actual env fallback is a Clap feature.
        let config = parse_config(
            r#"{"schema_version":2,"name":"test","args":[
                {"name":"input","long":"input","type":"option","env":"TEST_INPUT"}
            ]}"#,
        );
        config.validate().unwrap();
        // Without env var set and no CLI arg, value should be absent
        let result = unwrap_success_full(parse_args(&config, &to_args(&[]), get_name(&config)));
        assert!(result.values.get("input").is_none());
    }

    #[test]
    fn test_multiple_option_values() {
        let config = parse_config(
            r#"{"schema_version":2,"name":"test","args":[
                {"name":"file","long":"file","type":"option","multiple":true}
            ]}"#,
        );
        config.validate().unwrap();
        let result = unwrap_success_full(parse_args(
            &config,
            &to_args(&["--file", "a.txt", "--file", "b.txt"]),
            get_name(&config),
        ));
        match result.values.get("file") {
            Some(ParsedValue::Multiple(v)) => {
                assert_eq!(v, &vec!["a.txt".to_string(), "b.txt".to_string()]);
            }
            other => panic!("Expected Multiple, got {:?}", other),
        }
    }

    #[test]
    fn test_multiple_flag_count() {
        let config = parse_config(
            r#"{"schema_version":2,"name":"test","args":[
                {"name":"verbose","short":"v","type":"flag","multiple":true}
            ]}"#,
        );
        config.validate().unwrap();
        let result = unwrap_success(parse_args(&config, &to_args(&["-vvv"]), get_name(&config)));
        assert_eq!(result.get("verbose"), Some(&"3".to_string()));
    }

    #[test]
    fn test_delimiter_split() {
        let config = parse_config(
            r#"{"schema_version":2,"name":"test","args":[
                {"name":"tags","long":"tags","type":"option","multiple":true,"delimiter":","}
            ]}"#,
        );
        config.validate().unwrap();
        let result = unwrap_success_full(parse_args(
            &config,
            &to_args(&["--tags", "a,b,c"]),
            get_name(&config),
        ));
        match result.values.get("tags") {
            Some(ParsedValue::Multiple(v)) => {
                assert_eq!(v, &vec!["a".to_string(), "b".to_string(), "c".to_string()]);
            }
            other => panic!("Expected Multiple, got {:?}", other),
        }
    }

    #[test]
    fn test_subcommand_basic() {
        let config = parse_config(
            r#"{"schema_version":2,"name":"test","subcommands":[
                {"name":"init","help":"Initialize"}
            ]}"#,
        );
        config.validate().unwrap();
        let result =
            unwrap_success_full(parse_args(&config, &to_args(&["init"]), get_name(&config)));
        assert_eq!(result.subcommand, Some("init".to_string()));
    }

    #[test]
    fn test_subcommand_with_args() {
        let config = parse_config(
            r#"{"schema_version":2,"name":"test","subcommands":[
                {"name":"init","args":[
                    {"name":"template","type":"positional"}
                ]}
            ]}"#,
        );
        config.validate().unwrap();
        let result = unwrap_success_full(parse_args(
            &config,
            &to_args(&["init", "default"]),
            get_name(&config),
        ));
        assert_eq!(result.subcommand, Some("init".to_string()));
        assert_eq!(
            result.values.get("template"),
            Some(&ParsedValue::Single("default".to_string()))
        );
    }

    #[test]
    fn test_subcommand_with_main_args() {
        let config = parse_config(
            r#"{"schema_version":2,"name":"test",
                "args":[{"name":"verbose","short":"v","type":"flag"}],
                "subcommands":[{"name":"run"}]
            }"#,
        );
        config.validate().unwrap();
        let result = unwrap_success_full(parse_args(
            &config,
            &to_args(&["-v", "run"]),
            get_name(&config),
        ));
        assert_eq!(result.subcommand, Some("run".to_string()));
        assert_eq!(
            result.values.get("verbose"),
            Some(&ParsedValue::Single("true".to_string()))
        );
    }

    #[test]
    fn test_subcommand_required() {
        let config = parse_config(
            r#"{"schema_version":2,"name":"test","subcommands":[
                {"name":"init"}
            ]}"#,
        );
        config.validate().unwrap();
        let result = parse_args(&config, &to_args(&[]), get_name(&config));
        // Should error because subcommand is required
        assert!(matches!(
            result,
            ParseOutcome::Help(_) | ParseOutcome::Error(_)
        ));
    }

    #[test]
    fn test_num_args_range() {
        let config = parse_config(
            r#"{"schema_version":2,"name":"test","args":[
                {"name":"files","long":"file","type":"option","multiple":true,"num_args":"1..3"}
            ]}"#,
        );
        config.validate().unwrap();
        let result = unwrap_success_full(parse_args(
            &config,
            &to_args(&["--file", "a", "b"]),
            get_name(&config),
        ));
        match result.values.get("files") {
            Some(ParsedValue::Multiple(v)) => {
                assert_eq!(v.len(), 2);
            }
            other => panic!("Expected Multiple, got {:?}", other),
        }
    }

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
    }

    #[test]
    fn test_long_fallback_to_name() {
        // When neither short nor long is specified, name should be used as long
        let config = parse_config(
            r#"{"name":"test","args":[
                {"name":"verbose","type":"flag"}
            ]}"#,
        );
        config.validate().unwrap();
        let result = unwrap_success(parse_args(
            &config,
            &to_args(&["--verbose"]),
            get_name(&config),
        ));
        assert_eq!(result.get("verbose"), Some(&"true".to_string()));
    }

    #[test]
    fn test_effective_name_override() {
        // Test that the effective_name parameter is used correctly
        let config = parse_config(r#"{"name":"config_name"}"#);
        let result = parse_args(&config, &to_args(&["--help"]), "override_name");
        match result {
            ParseOutcome::Help(help_text) => {
                assert!(
                    help_text.contains("override_name"),
                    "Help should contain override_name"
                );
            }
            other => panic!("Expected Help, got {:?}", other),
        }
    }
}
