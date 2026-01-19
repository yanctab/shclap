//! Temporary file generation with shell export statements and special outputs.

use anyhow::Result;
use std::collections::HashMap;
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
pub fn generate_output(parsed: &HashMap<String, String>, prefix: &str) -> Result<PathBuf> {
    let mut file = NamedTempFile::new()?;

    for (name, value) in parsed {
        let var_name = format!("{}{}", prefix, to_shell_var_name(name));
        let escaped_value = escape_shell_value(value);
        writeln!(file, "export {}=\"{}\"", var_name, escaped_value)?;
    }

    // Persist the file (don't delete on drop)
    let path = file.into_temp_path().keep()?;
    Ok(path)
}

/// Generate the output content as a string (for testing).
pub fn generate_output_string(parsed: &HashMap<String, String>, prefix: &str) -> String {
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

    #[test]
    fn test_basic_output() {
        let parsed = make_map(&[("verbose", "true"), ("output", "file.txt")]);
        let output = generate_output_string(&parsed, "SHCLAP_");

        assert!(output.contains("export SHCLAP_OUTPUT=\"file.txt\""));
        assert!(output.contains("export SHCLAP_VERBOSE=\"true\""));
    }

    #[test]
    fn test_escape_dollar() {
        let parsed = make_map(&[("value", "$HOME/path")]);
        let output = generate_output_string(&parsed, "SHCLAP_");

        assert!(output.contains("export SHCLAP_VALUE=\"\\$HOME/path\""));
    }

    #[test]
    fn test_escape_backtick() {
        let parsed = make_map(&[("cmd", "`whoami`")]);
        let output = generate_output_string(&parsed, "SHCLAP_");

        assert!(output.contains("export SHCLAP_CMD=\"\\`whoami\\`\""));
    }

    #[test]
    fn test_escape_backslash() {
        let parsed = make_map(&[("path", "C:\\Users\\test")]);
        let output = generate_output_string(&parsed, "SHCLAP_");

        assert!(output.contains("export SHCLAP_PATH=\"C:\\\\Users\\\\test\""));
    }

    #[test]
    fn test_escape_double_quote() {
        let parsed = make_map(&[("msg", "say \"hello\"")]);
        let output = generate_output_string(&parsed, "SHCLAP_");

        assert!(output.contains("export SHCLAP_MSG=\"say \\\"hello\\\"\""));
    }

    #[test]
    fn test_escape_exclamation() {
        let parsed = make_map(&[("msg", "hello!")]);
        let output = generate_output_string(&parsed, "SHCLAP_");

        assert!(output.contains("export SHCLAP_MSG=\"hello\\!\""));
    }

    #[test]
    fn test_escape_newline() {
        let parsed = make_map(&[("text", "line1\nline2")]);
        let output = generate_output_string(&parsed, "SHCLAP_");

        assert!(output.contains("export SHCLAP_TEXT=\"line1\\nline2\""));
    }

    #[test]
    fn test_custom_prefix() {
        let parsed = make_map(&[("name", "test")]);
        let output = generate_output_string(&parsed, "MYAPP_");

        assert!(output.contains("export MYAPP_NAME=\"test\""));
    }

    #[test]
    fn test_empty_value() {
        let parsed = make_map(&[("empty", "")]);
        let output = generate_output_string(&parsed, "SHCLAP_");

        assert!(output.contains("export SHCLAP_EMPTY=\"\""));
    }

    #[test]
    fn test_value_with_spaces() {
        let parsed = make_map(&[("msg", "hello world")]);
        let output = generate_output_string(&parsed, "SHCLAP_");

        assert!(output.contains("export SHCLAP_MSG=\"hello world\""));
    }

    #[test]
    fn test_hyphenated_name() {
        let parsed = make_map(&[("my-option", "value")]);
        let output = generate_output_string(&parsed, "SHCLAP_");

        assert!(output.contains("export SHCLAP_MY_OPTION=\"value\""));
    }

    #[test]
    fn test_generate_output_creates_file() {
        let parsed = make_map(&[("test", "value")]);
        let path = generate_output(&parsed, "SHCLAP_").unwrap();

        assert!(path.exists());

        let contents = std::fs::read_to_string(&path).unwrap();
        assert!(contents.contains("export SHCLAP_TEST=\"value\""));

        // Clean up
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn test_complex_escaping() {
        let parsed = make_map(&[("complex", "$var \"quoted\" `cmd` \\path!")]);
        let output = generate_output_string(&parsed, "TEST_");

        assert!(
            output.contains("export TEST_COMPLEX=\"\\$var \\\"quoted\\\" \\`cmd\\` \\\\path\\!\"")
        );
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
}
