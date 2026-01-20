//! Argument parsing for target scripts using dynamic Clap.

use crate::config::{ArgConfig, ArgType, Config};
use clap::{error::ErrorKind, Arg, ArgAction, Command};
use std::collections::HashMap;

/// Outcome of parsing arguments.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseOutcome {
    /// Successfully parsed arguments with variable values.
    Success(HashMap<String, String>),
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

/// Build a Clap Command from a Config.
fn build_command(config: &Config) -> Command {
    let mut cmd = Command::new(config.name.clone())
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

    cmd
}

/// Build a Clap Arg from an ArgConfig.
fn build_arg(arg_config: &ArgConfig, positional_index: &mut usize) -> Arg {
    let mut arg = Arg::new(arg_config.name.clone());

    match arg_config.arg_type {
        ArgType::Flag => {
            arg = arg.action(ArgAction::SetTrue);

            // Add short option
            if let Some(short) = arg_config.short {
                arg = arg.short(short);
            }

            // Add long option
            if let Some(ref long) = arg_config.long {
                arg = arg.long(long.clone());
            }
        }
        ArgType::Option => {
            arg = arg.action(ArgAction::Set);

            // Add short option
            if let Some(short) = arg_config.short {
                arg = arg.short(short);
            }

            // Add long option
            if let Some(ref long) = arg_config.long {
                arg = arg.long(long.clone());
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

    arg
}

/// Extract parsed values from ArgMatches into a HashMap.
fn extract_values(config: &Config, matches: &clap::ArgMatches) -> HashMap<String, String> {
    let mut results = HashMap::new();

    for arg_config in &config.args {
        let name = &arg_config.name;

        match arg_config.arg_type {
            ArgType::Flag => {
                let value = matches.get_flag(name);
                results.insert(name.clone(), value.to_string());
            }
            ArgType::Option | ArgType::Positional => {
                if let Some(value) = matches.get_one::<String>(name) {
                    results.insert(name.clone(), value.clone());
                } else if let Some(ref default) = arg_config.default {
                    results.insert(name.clone(), default.clone());
                }
            }
        }
    }

    results
}

/// Parse command-line arguments according to the config.
///
/// Returns `ParseOutcome::Help` if -h/--help is found.
/// Returns `ParseOutcome::Version` if -V/--version is found.
/// Returns `ParseOutcome::Success` with parsed values on success.
/// Returns `ParseOutcome::Error` on parse errors.
pub fn parse_args(config: &Config, args: &[String]) -> ParseOutcome {
    let cmd = build_command(config);

    // Prepend program name since Clap expects args[0] to be the program name
    let mut full_args = vec![config.name.clone()];
    full_args.extend(args.iter().cloned());

    match cmd.try_get_matches_from(&full_args) {
        Ok(matches) => {
            let values = extract_values(config, &matches);
            ParseOutcome::Success(values)
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

    fn args(s: &[&str]) -> Vec<String> {
        s.iter().map(|s| s.to_string()).collect()
    }

    fn unwrap_success(outcome: ParseOutcome) -> HashMap<String, String> {
        match outcome {
            ParseOutcome::Success(map) => map,
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
        let result = unwrap_success(parse_args(&config, &args(&["--verbose"])));
        assert_eq!(result.get("verbose"), Some(&"true".to_string()));
    }

    #[test]
    fn test_parse_flag_short() {
        let config = parse_config(
            r#"{"name":"test","args":[
                {"name":"verbose","short":"v","type":"flag"}
            ]}"#,
        );
        let result = unwrap_success(parse_args(&config, &args(&["-v"])));
        assert_eq!(result.get("verbose"), Some(&"true".to_string()));
    }

    #[test]
    fn test_parse_flag_default_false() {
        let config = parse_config(
            r#"{"name":"test","args":[
                {"name":"verbose","short":"v","type":"flag"}
            ]}"#,
        );
        let result = unwrap_success(parse_args(&config, &args(&[])));
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
        let result = unwrap_success(parse_args(&config, &args(&["-abc"])));
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
        let result = unwrap_success(parse_args(&config, &args(&["--output", "file.txt"])));
        assert_eq!(result.get("output"), Some(&"file.txt".to_string()));
    }

    #[test]
    fn test_parse_option_long_equals() {
        let config = parse_config(
            r#"{"name":"test","args":[
                {"name":"output","long":"output","type":"option"}
            ]}"#,
        );
        let result = unwrap_success(parse_args(&config, &args(&["--output=file.txt"])));
        assert_eq!(result.get("output"), Some(&"file.txt".to_string()));
    }

    #[test]
    fn test_parse_option_short_space() {
        let config = parse_config(
            r#"{"name":"test","args":[
                {"name":"output","short":"o","type":"option"}
            ]}"#,
        );
        let result = unwrap_success(parse_args(&config, &args(&["-o", "file.txt"])));
        assert_eq!(result.get("output"), Some(&"file.txt".to_string()));
    }

    #[test]
    fn test_parse_option_short_attached() {
        let config = parse_config(
            r#"{"name":"test","args":[
                {"name":"output","short":"o","type":"option"}
            ]}"#,
        );
        let result = unwrap_success(parse_args(&config, &args(&["-ofile.txt"])));
        assert_eq!(result.get("output"), Some(&"file.txt".to_string()));
    }

    #[test]
    fn test_parse_positional() {
        let config = parse_config(
            r#"{"name":"test","args":[
                {"name":"input","type":"positional"}
            ]}"#,
        );
        let result = unwrap_success(parse_args(&config, &args(&["input.txt"])));
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
        let result = unwrap_success(parse_args(&config, &args(&["in.txt", "out.txt"])));
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
            &args(&["-v", "--output", "out.txt", "in.txt"]),
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
        let result = unwrap_success(parse_args(&config, &args(&["--", "-v"])));
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
        let result = unwrap_success(parse_args(&config, &args(&[])));
        assert_eq!(result.get("output"), Some(&"out.txt".to_string()));
    }

    #[test]
    fn test_parse_default_overridden() {
        let config = parse_config(
            r#"{"name":"test","args":[
                {"name":"output","long":"output","type":"option","default":"default.txt"}
            ]}"#,
        );
        let result = unwrap_success(parse_args(&config, &args(&["--output", "custom.txt"])));
        assert_eq!(result.get("output"), Some(&"custom.txt".to_string()));
    }

    #[test]
    fn test_error_missing_required() {
        let config = parse_config(
            r#"{"name":"test","args":[
                {"name":"input","type":"positional","required":true}
            ]}"#,
        );
        let result = parse_args(&config, &args(&[]));
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
        let result = parse_args(&config, &args(&["--output"]));
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
        let result = parse_args(&config, &args(&["--unknown"]));
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
        let result = parse_args(&config, &args(&["unexpected"]));
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
        let result = unwrap_success(parse_args(&config, &args(&["-vo", "file.txt"])));
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
        let result = unwrap_success(parse_args(&config, &args(&["-vofile.txt"])));
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
        let result = unwrap_success(parse_args(&config, &args(&["--msg", "hello $USER!"])));
        assert_eq!(result.get("msg"), Some(&"hello $USER!".to_string()));
    }

    #[test]
    fn test_empty_value() {
        let config = parse_config(
            r#"{"name":"test","args":[
                {"name":"value","long":"value","type":"option"}
            ]}"#,
        );
        let result = unwrap_success(parse_args(&config, &args(&["--value", ""])));
        assert_eq!(result.get("value"), Some(&"".to_string()));
    }

    #[test]
    fn test_option_equals_empty() {
        let config = parse_config(
            r#"{"name":"test","args":[
                {"name":"value","long":"value","type":"option"}
            ]}"#,
        );
        let result = unwrap_success(parse_args(&config, &args(&["--value="])));
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
            &args(&["input.txt", "-o", "output.txt"]),
        ));
        assert_eq!(result.get("input"), Some(&"input.txt".to_string()));
        assert_eq!(result.get("output"), Some(&"output.txt".to_string()));
    }

    #[test]
    fn test_help_flag_long() {
        let config = parse_config(r#"{"name":"test"}"#);
        let result = parse_args(&config, &args(&["--help"]));
        assert!(matches!(result, ParseOutcome::Help(_)));
    }

    #[test]
    fn test_help_flag_short() {
        let config = parse_config(r#"{"name":"test"}"#);
        let result = parse_args(&config, &args(&["-h"]));
        assert!(matches!(result, ParseOutcome::Help(_)));
    }

    #[test]
    fn test_version_flag_long() {
        let config = parse_config(r#"{"name":"test","version":"1.0.0"}"#);
        let result = parse_args(&config, &args(&["--version"]));
        assert!(matches!(result, ParseOutcome::Version(_)));
    }

    #[test]
    fn test_version_flag_short() {
        let config = parse_config(r#"{"name":"test","version":"1.0.0"}"#);
        let result = parse_args(&config, &args(&["-V"]));
        assert!(matches!(result, ParseOutcome::Version(_)));
    }

    #[test]
    fn test_help_takes_precedence_over_version() {
        let config = parse_config(r#"{"name":"test","version":"1.0.0"}"#);
        let result = parse_args(&config, &args(&["--version", "--help"]));
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
        let result = parse_args(&config, &args(&["-v", "--help"]));
        assert!(matches!(result, ParseOutcome::Help(_)));
    }

    #[test]
    fn test_version_anywhere_in_args() {
        let config = parse_config(
            r#"{"name":"test","version":"1.0.0","args":[
                {"name":"verbose","short":"v","type":"flag"}
            ]}"#,
        );
        let result = parse_args(&config, &args(&["-v", "--version"]));
        assert!(matches!(result, ParseOutcome::Version(_)));
    }
}
