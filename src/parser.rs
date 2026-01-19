//! Argument parsing for target scripts.

use crate::config::{ArgConfig, ArgType, Config};
use std::collections::HashMap;
use thiserror::Error;

/// Errors that can occur during argument parsing.
#[derive(Debug, Error)]
pub enum ParseError {
    #[error("missing required argument: {0}")]
    MissingRequired(String),

    #[error("missing value for option: {0}")]
    MissingValue(String),

    #[error("unknown option: {0}")]
    UnknownOption(String),

    #[error("unexpected positional argument: {0}")]
    UnexpectedPositional(String),
}

/// Outcome of parsing arguments.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseOutcome {
    /// Successfully parsed arguments with variable values.
    Success(HashMap<String, String>),
    /// User requested help (-h or --help).
    Help,
    /// User requested version (-V or --version).
    Version,
}

/// Result of parsing arguments (legacy type alias).
pub type ParseResult = Result<HashMap<String, String>, ParseError>;

/// Parse command-line arguments according to the config.
///
/// Returns `ParseOutcome::Help` if -h/--help is found anywhere in args.
/// Returns `ParseOutcome::Version` if -V/--version is found (and no help flag).
/// Otherwise returns `ParseOutcome::Success` with parsed values or an error.
pub fn parse_args(config: &Config, args: &[String]) -> Result<ParseOutcome, ParseError> {
    // Check for help/version flags first (anywhere in args)
    for arg in args {
        match arg.as_str() {
            "-h" | "--help" => return Ok(ParseOutcome::Help),
            _ => {}
        }
    }
    for arg in args {
        match arg.as_str() {
            "-V" | "--version" => return Ok(ParseOutcome::Version),
            _ => {}
        }
    }

    let mut parser = Parser::new(config);
    parser.parse(args).map(ParseOutcome::Success)
}

/// Internal parser state.
struct Parser<'a> {
    config: &'a Config,
    results: HashMap<String, String>,
    positional_index: usize,
}

impl<'a> Parser<'a> {
    fn new(config: &'a Config) -> Self {
        Self {
            config,
            results: HashMap::new(),
            positional_index: 0,
        }
    }

    fn parse(&mut self, args: &[String]) -> ParseResult {
        let mut args_iter = args.iter().peekable();
        let mut parsing_options = true;

        while let Some(arg) = args_iter.next() {
            if parsing_options && arg == "--" {
                // Stop parsing options, everything after is positional
                parsing_options = false;
                continue;
            }

            if parsing_options && arg.starts_with("--") {
                // Long option
                self.parse_long_option(arg, &mut args_iter)?;
            } else if parsing_options && arg.starts_with('-') && arg.len() > 1 {
                // Short option(s)
                self.parse_short_options(arg, &mut args_iter)?;
            } else {
                // Positional argument
                self.parse_positional(arg)?;
            }
        }

        // Apply defaults and validate required args
        self.apply_defaults();
        self.validate_required()?;

        Ok(self.results.clone())
    }

    fn parse_long_option(
        &mut self,
        arg: &str,
        args_iter: &mut std::iter::Peekable<std::slice::Iter<String>>,
    ) -> Result<(), ParseError> {
        let option_str = &arg[2..]; // Strip "--"

        // Check for --option=value format
        let (name, inline_value) = if let Some(eq_pos) = option_str.find('=') {
            let (n, v) = option_str.split_at(eq_pos);
            (n, Some(&v[1..])) // Skip the '='
        } else {
            (option_str, None)
        };

        // Find matching arg config
        let arg_config = self
            .config
            .args
            .iter()
            .find(|a| a.long.as_deref() == Some(name))
            .ok_or_else(|| ParseError::UnknownOption(format!("--{}", name)))?;

        match arg_config.arg_type {
            ArgType::Flag => {
                self.results
                    .insert(arg_config.name.clone(), "true".to_string());
            }
            ArgType::Option => {
                let value = if let Some(v) = inline_value {
                    v.to_string()
                } else {
                    args_iter
                        .next()
                        .ok_or_else(|| ParseError::MissingValue(format!("--{}", name)))?
                        .clone()
                };
                self.results.insert(arg_config.name.clone(), value);
            }
            ArgType::Positional => {
                // Positional args don't have long options, this shouldn't happen
                // due to config validation, but handle it gracefully
                return Err(ParseError::UnknownOption(format!("--{}", name)));
            }
        }

        Ok(())
    }

    fn parse_short_options(
        &mut self,
        arg: &str,
        args_iter: &mut std::iter::Peekable<std::slice::Iter<String>>,
    ) -> Result<(), ParseError> {
        let chars: Vec<char> = arg[1..].chars().collect(); // Strip "-"

        for (i, c) in chars.iter().enumerate() {
            let arg_config = self
                .config
                .args
                .iter()
                .find(|a| a.short == Some(*c))
                .ok_or_else(|| ParseError::UnknownOption(format!("-{}", c)))?;

            match arg_config.arg_type {
                ArgType::Flag => {
                    self.results
                        .insert(arg_config.name.clone(), "true".to_string());
                }
                ArgType::Option => {
                    // For options, the value can be:
                    // 1. The rest of this arg (e.g., -ofile.txt)
                    // 2. The next arg (e.g., -o file.txt)
                    let remaining: String = chars[i + 1..].iter().collect();
                    let value = if !remaining.is_empty() {
                        remaining
                    } else {
                        args_iter
                            .next()
                            .ok_or_else(|| ParseError::MissingValue(format!("-{}", c)))?
                            .clone()
                    };
                    self.results.insert(arg_config.name.clone(), value);
                    // We've consumed the rest of the chars as value
                    return Ok(());
                }
                ArgType::Positional => {
                    return Err(ParseError::UnknownOption(format!("-{}", c)));
                }
            }
        }

        Ok(())
    }

    fn parse_positional(&mut self, arg: &str) -> Result<(), ParseError> {
        let positionals: Vec<&ArgConfig> = self
            .config
            .args
            .iter()
            .filter(|a| a.arg_type == ArgType::Positional)
            .collect();

        if self.positional_index >= positionals.len() {
            return Err(ParseError::UnexpectedPositional(arg.to_string()));
        }

        let arg_config = positionals[self.positional_index];
        self.results
            .insert(arg_config.name.clone(), arg.to_string());
        self.positional_index += 1;

        Ok(())
    }

    fn apply_defaults(&mut self) {
        for arg in &self.config.args {
            if !self.results.contains_key(&arg.name) {
                if let Some(ref default) = arg.default {
                    self.results.insert(arg.name.clone(), default.clone());
                } else if arg.arg_type == ArgType::Flag {
                    // Flags default to "false" if not set
                    self.results.insert(arg.name.clone(), "false".to_string());
                }
            }
        }
    }

    fn validate_required(&self) -> Result<(), ParseError> {
        for arg in &self.config.args {
            if arg.required && !self.results.contains_key(&arg.name) {
                return Err(ParseError::MissingRequired(arg.name.clone()));
            }
        }
        Ok(())
    }
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
        let result = unwrap_success(parse_args(&config, &args(&["--verbose"])).unwrap());
        assert_eq!(result.get("verbose"), Some(&"true".to_string()));
    }

    #[test]
    fn test_parse_flag_short() {
        let config = parse_config(
            r#"{"name":"test","args":[
                {"name":"verbose","short":"v","type":"flag"}
            ]}"#,
        );
        let result = unwrap_success(parse_args(&config, &args(&["-v"])).unwrap());
        assert_eq!(result.get("verbose"), Some(&"true".to_string()));
    }

    #[test]
    fn test_parse_flag_default_false() {
        let config = parse_config(
            r#"{"name":"test","args":[
                {"name":"verbose","short":"v","type":"flag"}
            ]}"#,
        );
        let result = unwrap_success(parse_args(&config, &args(&[])).unwrap());
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
        let result = unwrap_success(parse_args(&config, &args(&["-abc"])).unwrap());
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
        let result = unwrap_success(parse_args(&config, &args(&["--output", "file.txt"])).unwrap());
        assert_eq!(result.get("output"), Some(&"file.txt".to_string()));
    }

    #[test]
    fn test_parse_option_long_equals() {
        let config = parse_config(
            r#"{"name":"test","args":[
                {"name":"output","long":"output","type":"option"}
            ]}"#,
        );
        let result = unwrap_success(parse_args(&config, &args(&["--output=file.txt"])).unwrap());
        assert_eq!(result.get("output"), Some(&"file.txt".to_string()));
    }

    #[test]
    fn test_parse_option_short_space() {
        let config = parse_config(
            r#"{"name":"test","args":[
                {"name":"output","short":"o","type":"option"}
            ]}"#,
        );
        let result = unwrap_success(parse_args(&config, &args(&["-o", "file.txt"])).unwrap());
        assert_eq!(result.get("output"), Some(&"file.txt".to_string()));
    }

    #[test]
    fn test_parse_option_short_attached() {
        let config = parse_config(
            r#"{"name":"test","args":[
                {"name":"output","short":"o","type":"option"}
            ]}"#,
        );
        let result = unwrap_success(parse_args(&config, &args(&["-ofile.txt"])).unwrap());
        assert_eq!(result.get("output"), Some(&"file.txt".to_string()));
    }

    #[test]
    fn test_parse_positional() {
        let config = parse_config(
            r#"{"name":"test","args":[
                {"name":"input","type":"positional"}
            ]}"#,
        );
        let result = unwrap_success(parse_args(&config, &args(&["input.txt"])).unwrap());
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
        let result = unwrap_success(parse_args(&config, &args(&["in.txt", "out.txt"])).unwrap());
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
        let result = unwrap_success(
            parse_args(&config, &args(&["-v", "--output", "out.txt", "in.txt"])).unwrap(),
        );
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
        let result = unwrap_success(parse_args(&config, &args(&["--", "-v"])).unwrap());
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
        let result = unwrap_success(parse_args(&config, &args(&[])).unwrap());
        assert_eq!(result.get("output"), Some(&"out.txt".to_string()));
    }

    #[test]
    fn test_parse_default_overridden() {
        let config = parse_config(
            r#"{"name":"test","args":[
                {"name":"output","long":"output","type":"option","default":"default.txt"}
            ]}"#,
        );
        let result =
            unwrap_success(parse_args(&config, &args(&["--output", "custom.txt"])).unwrap());
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
        assert!(matches!(result, Err(ParseError::MissingRequired(_))));
    }

    #[test]
    fn test_error_missing_value() {
        let config = parse_config(
            r#"{"name":"test","args":[
                {"name":"output","long":"output","type":"option"}
            ]}"#,
        );
        let result = parse_args(&config, &args(&["--output"]));
        assert!(matches!(result, Err(ParseError::MissingValue(_))));
    }

    #[test]
    fn test_error_unknown_option() {
        let config = parse_config(r#"{"name":"test","args":[]}"#);
        let result = parse_args(&config, &args(&["--unknown"]));
        assert!(matches!(result, Err(ParseError::UnknownOption(_))));
    }

    #[test]
    fn test_error_unexpected_positional() {
        let config = parse_config(r#"{"name":"test","args":[]}"#);
        let result = parse_args(&config, &args(&["unexpected"]));
        assert!(matches!(result, Err(ParseError::UnexpectedPositional(_))));
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
        let result = unwrap_success(parse_args(&config, &args(&["-vo", "file.txt"])).unwrap());
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
        let result = unwrap_success(parse_args(&config, &args(&["-vofile.txt"])).unwrap());
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
        let result =
            unwrap_success(parse_args(&config, &args(&["--msg", "hello $USER!"])).unwrap());
        assert_eq!(result.get("msg"), Some(&"hello $USER!".to_string()));
    }

    #[test]
    fn test_empty_value() {
        let config = parse_config(
            r#"{"name":"test","args":[
                {"name":"value","long":"value","type":"option"}
            ]}"#,
        );
        let result = unwrap_success(parse_args(&config, &args(&["--value", ""])).unwrap());
        assert_eq!(result.get("value"), Some(&"".to_string()));
    }

    #[test]
    fn test_option_equals_empty() {
        let config = parse_config(
            r#"{"name":"test","args":[
                {"name":"value","long":"value","type":"option"}
            ]}"#,
        );
        let result = unwrap_success(parse_args(&config, &args(&["--value="])).unwrap());
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
        let result =
            unwrap_success(parse_args(&config, &args(&["input.txt", "-o", "output.txt"])).unwrap());
        assert_eq!(result.get("input"), Some(&"input.txt".to_string()));
        assert_eq!(result.get("output"), Some(&"output.txt".to_string()));
    }

    #[test]
    fn test_help_flag_long() {
        let config = parse_config(r#"{"name":"test"}"#);
        let result = parse_args(&config, &args(&["--help"])).unwrap();
        assert_eq!(result, ParseOutcome::Help);
    }

    #[test]
    fn test_help_flag_short() {
        let config = parse_config(r#"{"name":"test"}"#);
        let result = parse_args(&config, &args(&["-h"])).unwrap();
        assert_eq!(result, ParseOutcome::Help);
    }

    #[test]
    fn test_version_flag_long() {
        let config = parse_config(r#"{"name":"test"}"#);
        let result = parse_args(&config, &args(&["--version"])).unwrap();
        assert_eq!(result, ParseOutcome::Version);
    }

    #[test]
    fn test_version_flag_short() {
        let config = parse_config(r#"{"name":"test"}"#);
        let result = parse_args(&config, &args(&["-V"])).unwrap();
        assert_eq!(result, ParseOutcome::Version);
    }

    #[test]
    fn test_help_takes_precedence_over_version() {
        let config = parse_config(r#"{"name":"test"}"#);
        let result = parse_args(&config, &args(&["--version", "--help"])).unwrap();
        assert_eq!(result, ParseOutcome::Help);
    }

    #[test]
    fn test_help_anywhere_in_args() {
        let config = parse_config(
            r#"{"name":"test","args":[
                {"name":"verbose","short":"v","type":"flag"}
            ]}"#,
        );
        let result = parse_args(&config, &args(&["-v", "--help"])).unwrap();
        assert_eq!(result, ParseOutcome::Help);
    }

    #[test]
    fn test_version_anywhere_in_args() {
        let config = parse_config(
            r#"{"name":"test","args":[
                {"name":"verbose","short":"v","type":"flag"}
            ]}"#,
        );
        let result = parse_args(&config, &args(&["-v", "--version"])).unwrap();
        assert_eq!(result, ParseOutcome::Version);
    }
}
