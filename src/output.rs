//! Temporary file generation with shell export statements and special outputs.

use crate::config::{ArgType, Config};
use crate::parser::ParsedValue;
use anyhow::Result;
use std::collections::HashMap;
use std::env;
use std::io::Write;
use std::path::PathBuf;
use tempfile::NamedTempFile;

/// Heredoc delimiter for help output.
const HELP_DELIMITER: &str = "SHCLAP_HELP";
/// Heredoc delimiter for version output.
const VERSION_DELIMITER: &str = "SHCLAP_VERSION";

/// Escape a string for safe use in a shell double-quoted context.
///
/// Escapes: $, `, \, ", and !
fn escape_shell_value(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for c in value.chars() {
        match c {
            '$' => escaped.push_str("\\$"),
            '`' => escaped.push_str("\\`"),
            '\\' => escaped.push_str("\\\\"),
            '"' => escaped.push_str("\\\""),
            '!' => escaped.push_str("\\!"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            _ => escaped.push(c),
        }
    }
    escaped
}

/// Convert an argument name to a valid shell variable name.
///
/// Converts to uppercase and replaces hyphens with underscores.
fn to_shell_var_name(name: &str) -> String {
    name.to_uppercase().replace('-', "_")
}

/// Generate a temporary file with shell export statements.
///
/// Returns the path to the temporary file. The file will persist
/// until the process exits or it's manually deleted.
pub fn generate_output(
    parsed: &HashMap<String, ParsedValue>,
    prefix: &str,
    subcommand: Option<&str>,
) -> Result<PathBuf> {
    let content = generate_output_string(parsed, prefix, subcommand);
    write_temp_file(&content)
}

/// Generate the output content as a string (for testing).
pub fn generate_output_string(
    parsed: &HashMap<String, ParsedValue>,
    prefix: &str,
    subcommand: Option<&str>,
) -> String {
    let mut output = String::new();

    // Output subcommand first if present
    if let Some(subcmd) = subcommand {
        output.push_str(&format!(
            "export {}SUBCOMMAND=\"{}\"\n",
            prefix,
            escape_shell_value(subcmd)
        ));
    }

    // Sort keys for deterministic output
    let mut keys: Vec<_> = parsed.keys().collect();
    keys.sort();

    for name in keys {
        let value = &parsed[name];
        let var_name = format!("{}{}", prefix, to_shell_var_name(name));

        match value {
            ParsedValue::Single(s) => {
                let escaped_value = escape_shell_value(s);
                output.push_str(&format!("export {}=\"{}\"\n", var_name, escaped_value));
            }
            ParsedValue::Multiple(values) => {
                // Output as bash array: export VAR=("val1" "val2" "val3")
                let escaped: Vec<String> = values
                    .iter()
                    .map(|v| format!("\"{}\"", escape_shell_value(v)))
                    .collect();
                output.push_str(&format!("export {}=({})\n", var_name, escaped.join(" ")));
            }
        }
    }

    output
}

/// Generate output using legacy HashMap<String, String> format.
/// For backward compatibility with existing code.
pub fn generate_output_legacy(parsed: &HashMap<String, String>, prefix: &str) -> Result<PathBuf> {
    let content = generate_output_string_legacy(parsed, prefix);
    write_temp_file(&content)
}

/// Generate the output content as a string using legacy format (for testing).
pub fn generate_output_string_legacy(parsed: &HashMap<String, String>, prefix: &str) -> String {
    let mut output = String::new();

    // Sort keys for deterministic output
    let mut keys: Vec<_> = parsed.keys().collect();
    keys.sort();

    for name in keys {
        let value = &parsed[name];
        let var_name = format!("{}{}", prefix, to_shell_var_name(name));
        let escaped_value = escape_shell_value(value);
        output.push_str(&format!("export {}=\"{}\"\n", var_name, escaped_value));
    }

    output
}

/// Generate an error output file.
///
/// When sourced, the file will print the error message to stderr and exit 1.
pub fn generate_error_output(message: &str) -> Result<PathBuf> {
    let content = generate_error_string(message);
    write_temp_file(&content)
}

/// Generate an error output as a string (for testing).
pub fn generate_error_string(message: &str) -> String {
    // Escape the message for safe use in double quotes
    let escaped = escape_shell_value(message);
    format!("echo \"shclap: {}\" >&2\nexit 1\n", escaped)
}

/// Generate a help output file.
///
/// When sourced, the file will print the help text and exit 0.
pub fn generate_help_output(help_text: &str) -> Result<PathBuf> {
    let content = generate_help_output_string(help_text);
    write_temp_file(&content)
}

/// Generate a help output as a string (for testing).
pub fn generate_help_output_string(help_text: &str) -> String {
    format!(
        "cat <<'{delimiter}'\n{text}{delimiter}\nexit 0\n",
        delimiter = HELP_DELIMITER,
        text = help_text
    )
}

/// Generate a version output file.
///
/// When sourced, the file will print the version and exit 0.
pub fn generate_version_output(version_text: &str) -> Result<PathBuf> {
    let content = generate_version_output_string(version_text);
    write_temp_file(&content)
}

/// Generate a version output as a string (for testing).
pub fn generate_version_output_string(version_text: &str) -> String {
    format!(
        "cat <<'{delimiter}'\n{text}{delimiter}\nexit 0\n",
        delimiter = VERSION_DELIMITER,
        text = version_text
    )
}

/// Generate a reconstructed command line from environment variables.
///
/// Reads the current environment variables (set by sourcing shclap's output)
/// and reconstructs how the script was called. This is useful for logging
/// or debugging.
///
/// # Arguments
/// * `config` - The script's configuration
/// * `name` - The script name to display
/// * `prefix` - The environment variable prefix used
///
/// # Returns
/// A string like: `scriptname --flag --option=value positional`
pub fn generate_print(config: &Config, name: &str, prefix: &str) -> String {
    let mut parts: Vec<String> = vec![name.to_string()];
    let mut positionals: Vec<String> = Vec::new();

    // Process all args from config
    for arg in &config.args {
        let var_name = format!("{}{}", prefix, to_shell_var_name(&arg.name));

        if let Ok(value) = env::var(&var_name) {
            match arg.arg_type {
                ArgType::Flag => {
                    // For flags, only add if value is "true" or a count > 0
                    if value == "true" {
                        // Use long form if available, otherwise short
                        if let Some(ref long) = arg.long {
                            parts.push(format!("--{}", long));
                        } else if let Some(ref long) = arg.effective_long() {
                            parts.push(format!("--{}", long));
                        } else if let Some(short) = arg.short {
                            parts.push(format!("-{}", short));
                        }
                    } else if let Ok(count) = value.parse::<u32>() {
                        // Multiple flag (count)
                        if count > 0 {
                            if let Some(short) = arg.short {
                                // Output as -vvv for count=3
                                parts
                                    .push(format!("-{}", short.to_string().repeat(count as usize)));
                            } else if let Some(ref long) = arg.long {
                                // Repeat the flag
                                for _ in 0..count {
                                    parts.push(format!("--{}", long));
                                }
                            } else if let Some(ref long) = arg.effective_long() {
                                for _ in 0..count {
                                    parts.push(format!("--{}", long));
                                }
                            }
                        }
                    }
                }
                ArgType::Option => {
                    if !value.is_empty() {
                        // Use long form with = syntax
                        if let Some(ref long) = arg.long {
                            parts.push(format!("--{}={}", long, shell_quote(&value)));
                        } else if let Some(ref long) = arg.effective_long() {
                            parts.push(format!("--{}={}", long, shell_quote(&value)));
                        } else if let Some(short) = arg.short {
                            parts.push(format!("-{}", short));
                            parts.push(shell_quote(&value));
                        }
                    }
                }
                ArgType::Positional => {
                    if !value.is_empty() {
                        positionals.push(shell_quote(&value));
                    }
                }
            }
        }
    }

    // Add positionals at the end
    parts.extend(positionals);

    parts.join(" ")
}

/// Quote a value for shell if it contains special characters.
fn shell_quote(value: &str) -> String {
    if value.is_empty() || value.contains(|c: char| c.is_whitespace() || "\"'$`\\!".contains(c)) {
        // Use single quotes, escaping any single quotes in the value
        format!("'{}'", value.replace('\'', "'\\''"))
    } else {
        value.to_string()
    }
}

/// Write content to a temporary file and return its path.
fn write_temp_file(content: &str) -> Result<PathBuf> {
    let mut file = NamedTempFile::new()?;
    file.write_all(content.as_bytes())?;
    let path = file.into_temp_path().keep()?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_map(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    fn make_parsed_map(pairs: &[(&str, ParsedValue)]) -> HashMap<String, ParsedValue> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.clone()))
            .collect()
    }

    #[test]
    fn test_basic_output() {
        let parsed = make_map(&[("verbose", "true"), ("output", "file.txt")]);
        let output = generate_output_string_legacy(&parsed, "SHCLAP_");

        assert!(output.contains("export SHCLAP_OUTPUT=\"file.txt\""));
        assert!(output.contains("export SHCLAP_VERBOSE=\"true\""));
    }

    #[test]
    fn test_escape_dollar() {
        let parsed = make_map(&[("value", "$HOME/path")]);
        let output = generate_output_string_legacy(&parsed, "SHCLAP_");

        assert!(output.contains("export SHCLAP_VALUE=\"\\$HOME/path\""));
    }

    #[test]
    fn test_escape_backtick() {
        let parsed = make_map(&[("cmd", "`whoami`")]);
        let output = generate_output_string_legacy(&parsed, "SHCLAP_");

        assert!(output.contains("export SHCLAP_CMD=\"\\`whoami\\`\""));
    }

    #[test]
    fn test_escape_backslash() {
        let parsed = make_map(&[("path", "C:\\Users\\test")]);
        let output = generate_output_string_legacy(&parsed, "SHCLAP_");

        assert!(output.contains("export SHCLAP_PATH=\"C:\\\\Users\\\\test\""));
    }

    #[test]
    fn test_escape_double_quote() {
        let parsed = make_map(&[("msg", "say \"hello\"")]);
        let output = generate_output_string_legacy(&parsed, "SHCLAP_");

        assert!(output.contains("export SHCLAP_MSG=\"say \\\"hello\\\"\""));
    }

    #[test]
    fn test_escape_exclamation() {
        let parsed = make_map(&[("msg", "hello!")]);
        let output = generate_output_string_legacy(&parsed, "SHCLAP_");

        assert!(output.contains("export SHCLAP_MSG=\"hello\\!\""));
    }

    #[test]
    fn test_escape_newline() {
        let parsed = make_map(&[("text", "line1\nline2")]);
        let output = generate_output_string_legacy(&parsed, "SHCLAP_");

        assert!(output.contains("export SHCLAP_TEXT=\"line1\\nline2\""));
    }

    #[test]
    fn test_custom_prefix() {
        let parsed = make_map(&[("name", "test")]);
        let output = generate_output_string_legacy(&parsed, "MYAPP_");

        assert!(output.contains("export MYAPP_NAME=\"test\""));
    }

    #[test]
    fn test_empty_value() {
        let parsed = make_map(&[("empty", "")]);
        let output = generate_output_string_legacy(&parsed, "SHCLAP_");

        assert!(output.contains("export SHCLAP_EMPTY=\"\""));
    }

    #[test]
    fn test_value_with_spaces() {
        let parsed = make_map(&[("msg", "hello world")]);
        let output = generate_output_string_legacy(&parsed, "SHCLAP_");

        assert!(output.contains("export SHCLAP_MSG=\"hello world\""));
    }

    #[test]
    fn test_hyphenated_name() {
        let parsed = make_map(&[("my-option", "value")]);
        let output = generate_output_string_legacy(&parsed, "SHCLAP_");

        assert!(output.contains("export SHCLAP_MY_OPTION=\"value\""));
    }

    #[test]
    fn test_generate_output_creates_file() {
        let parsed = make_parsed_map(&[("test", ParsedValue::Single("value".to_string()))]);
        let path = generate_output(&parsed, "SHCLAP_", None).unwrap();

        assert!(path.exists());

        let contents = std::fs::read_to_string(&path).unwrap();
        assert!(contents.contains("export SHCLAP_TEST=\"value\""));

        // Clean up
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn test_complex_escaping() {
        let parsed = make_map(&[("complex", "$var \"quoted\" `cmd` \\path!")]);
        let output = generate_output_string_legacy(&parsed, "TEST_");

        assert!(
            output.contains("export TEST_COMPLEX=\"\\$var \\\"quoted\\\" \\`cmd\\` \\\\path\\!\"")
        );
    }

    // Schema v2 tests

    #[test]
    fn test_single_value_output() {
        let parsed = make_parsed_map(&[
            ("verbose", ParsedValue::Single("true".to_string())),
            ("output", ParsedValue::Single("file.txt".to_string())),
        ]);
        let output = generate_output_string(&parsed, "SHCLAP_", None);

        assert!(output.contains("export SHCLAP_OUTPUT=\"file.txt\""));
        assert!(output.contains("export SHCLAP_VERBOSE=\"true\""));
    }

    #[test]
    fn test_multiple_values_array_output() {
        let parsed = make_parsed_map(&[(
            "files",
            ParsedValue::Multiple(vec![
                "a.txt".to_string(),
                "b.txt".to_string(),
                "c.txt".to_string(),
            ]),
        )]);
        let output = generate_output_string(&parsed, "SHCLAP_", None);

        assert!(output.contains("export SHCLAP_FILES=(\"a.txt\" \"b.txt\" \"c.txt\")"));
    }

    #[test]
    fn test_multiple_values_with_escaping() {
        let parsed = make_parsed_map(&[(
            "files",
            ParsedValue::Multiple(vec![
                "$HOME/a.txt".to_string(),
                "file with spaces".to_string(),
            ]),
        )]);
        let output = generate_output_string(&parsed, "SHCLAP_", None);

        assert!(output.contains("export SHCLAP_FILES=(\"\\$HOME/a.txt\" \"file with spaces\")"));
    }

    #[test]
    fn test_subcommand_output() {
        let parsed = make_parsed_map(&[("template", ParsedValue::Single("default".to_string()))]);
        let output = generate_output_string(&parsed, "SHCLAP_", Some("init"));

        assert!(output.contains("export SHCLAP_SUBCOMMAND=\"init\""));
        assert!(output.contains("export SHCLAP_TEMPLATE=\"default\""));
    }

    #[test]
    fn test_subcommand_first_in_output() {
        let parsed = make_parsed_map(&[("verbose", ParsedValue::Single("true".to_string()))]);
        let output = generate_output_string(&parsed, "SHCLAP_", Some("run"));

        // Subcommand should be first
        let subcmd_pos = output.find("SUBCOMMAND").unwrap();
        let verbose_pos = output.find("VERBOSE").unwrap();
        assert!(subcmd_pos < verbose_pos);
    }

    #[test]
    fn test_mixed_single_and_multiple() {
        let parsed = make_parsed_map(&[
            ("verbose", ParsedValue::Single("true".to_string())),
            (
                "files",
                ParsedValue::Multiple(vec!["a.txt".to_string(), "b.txt".to_string()]),
            ),
        ]);
        let output = generate_output_string(&parsed, "SHCLAP_", None);

        assert!(output.contains("export SHCLAP_VERBOSE=\"true\""));
        assert!(output.contains("export SHCLAP_FILES=(\"a.txt\" \"b.txt\")"));
    }

    #[test]
    fn test_generate_error_string() {
        let output = generate_error_string("unknown option: --foo");
        assert!(output.contains("echo \"shclap: unknown option: --foo\" >&2"));
        assert!(output.contains("exit 1"));
    }

    #[test]
    fn test_generate_error_string_escapes_special_chars() {
        let output = generate_error_string("bad value: $HOME `test`");
        assert!(output.contains("\\$HOME"));
        assert!(output.contains("\\`test\\`"));
        assert!(output.contains("exit 1"));
    }

    #[test]
    fn test_generate_help_output_string() {
        let help = "myapp v1.0.0\nA test app\n\nUSAGE:\n    myapp [OPTIONS]\n";
        let output = generate_help_output_string(help);

        assert!(output.starts_with("cat <<'SHCLAP_HELP'\n"));
        assert!(output.contains("myapp v1.0.0"));
        assert!(output.contains("USAGE:"));
        assert!(output.ends_with("SHCLAP_HELP\nexit 0\n"));
    }

    #[test]
    fn test_generate_version_output_string() {
        let version = "myapp 1.0.0\n";
        let output = generate_version_output_string(version);

        assert!(output.starts_with("cat <<'SHCLAP_VERSION'\n"));
        assert!(output.contains("myapp 1.0.0"));
        assert!(output.ends_with("SHCLAP_VERSION\nexit 0\n"));
    }

    #[test]
    fn test_generate_error_output_creates_file() {
        let path = generate_error_output("test error").unwrap();
        assert!(path.exists());

        let contents = std::fs::read_to_string(&path).unwrap();
        assert!(contents.contains("shclap: test error"));
        assert!(contents.contains("exit 1"));

        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn test_generate_help_output_creates_file() {
        let path = generate_help_output("test help text\n").unwrap();
        assert!(path.exists());

        let contents = std::fs::read_to_string(&path).unwrap();
        assert!(contents.contains("test help text"));
        assert!(contents.contains("exit 0"));

        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn test_generate_version_output_creates_file() {
        let path = generate_version_output("myapp 1.0.0\n").unwrap();
        assert!(path.exists());

        let contents = std::fs::read_to_string(&path).unwrap();
        assert!(contents.contains("myapp 1.0.0"));
        assert!(contents.contains("exit 0"));

        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn test_generate_print_basic() {
        use crate::config::Config;

        let config = Config::from_json(
            r#"{
            "name": "myapp",
            "args": [
                {"name": "verbose", "short": "v", "type": "flag"},
                {"name": "output", "short": "o", "type": "option"},
                {"name": "input", "type": "positional"}
            ]
        }"#,
        )
        .unwrap();

        // Set environment variables
        env::set_var("TEST_VERBOSE", "true");
        env::set_var("TEST_OUTPUT", "file.txt");
        env::set_var("TEST_INPUT", "input.txt");

        let result = generate_print(&config, "myapp", "TEST_");

        // Clean up
        env::remove_var("TEST_VERBOSE");
        env::remove_var("TEST_OUTPUT");
        env::remove_var("TEST_INPUT");

        assert!(result.starts_with("myapp"));
        assert!(result.contains("--verbose") || result.contains("-v"));
        assert!(result.contains("--output=file.txt") || result.contains("-o"));
        assert!(result.contains("input.txt"));
    }

    #[test]
    fn test_generate_print_no_values() {
        use crate::config::Config;

        let config = Config::from_json(
            r#"{
            "name": "myapp",
            "args": [
                {"name": "verbose", "short": "v", "type": "flag"}
            ]
        }"#,
        )
        .unwrap();

        // Ensure var is not set
        env::remove_var("EMPTY_VERBOSE");

        let result = generate_print(&config, "myapp", "EMPTY_");

        assert_eq!(result, "myapp");
    }

    #[test]
    fn test_generate_print_special_chars() {
        use crate::config::Config;

        let config = Config::from_json(
            r#"{
            "name": "myapp",
            "args": [
                {"name": "path", "type": "option"}
            ]
        }"#,
        )
        .unwrap();

        // Set a value with spaces
        env::set_var("SPECIAL_PATH", "path with spaces");

        let result = generate_print(&config, "myapp", "SPECIAL_");

        env::remove_var("SPECIAL_PATH");

        assert!(result.contains("'path with spaces'"));
    }
}
