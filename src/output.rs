//! Temporary file generation with shell export statements and special outputs.

use crate::config::{ArgConfig, ArgType, Config, ContainerConfig};
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

/// Quote a value for safe use in generated shell code.
///
/// Uses an allowlist: a value passes through bare only when every character is
/// inert to the shell. Anything else is single-quoted, so word splitting,
/// globbing, and command substitution cannot happen when the generated file is
/// sourced. A denylist is not enough here - `a;id` contains no whitespace and
/// no quoting metacharacter, but still runs a command.
fn shell_quote(value: &str) -> String {
    fn is_shell_safe(c: char) -> bool {
        c.is_ascii_alphanumeric() || "-_./:=@%+,".contains(c)
    }

    if !value.is_empty() && value.chars().all(is_shell_safe) {
        value.to_string()
    } else {
        // Single quotes suppress every expansion; the only character that needs
        // care inside them is the single quote itself.
        format!("'{}'", value.replace('\'', "'\\''"))
    }
}

/// Generate the shell fragment that re-execs the calling script inside a container.
///
/// Emits a script that:
/// 1. Resolves the calling script and shclap binary paths
/// 2. Checks the container runtime is available
/// 3. Re-execs the script inside the container with the required volume mounts
/// 4. Forwards prefix-matching and explicitly-named environment variables
pub fn generate_container_reexec_string(container: &ContainerConfig, config: &Config) -> String {
    use crate::config::PullPolicy;
    use std::collections::HashSet;

    let rt = &container.runtime;
    let mut s = String::new();
    s.push_str("_shclap_script=$(readlink -f \"$0\")\n");
    s.push_str("_shclap_bin=$(readlink -f \"$(command -v shclap)\")\n");
    s.push_str(&format!(
        "command -v {rt} >/dev/null 2>&1 || {{ echo \"shclap: container runtime '{rt}' not found\" >&2; exit 127; }}\n"
    ));
    s.push_str(&format!(
        "echo \"shclap: bootstrapping into {rt}:{image}\" >&2\n",
        image = container.image
    ));
    s.push_str("set -x\n");
    s.push_str(&format!("exec {rt} run --rm \\\n"));

    // Emit --pull flag based on PullPolicy
    let pull_value = match config.pull_policy {
        PullPolicy::Always => "always",
        PullPolicy::Never => "never",
        PullPolicy::IfNotPresent => "missing",
    };
    s.push_str(&format!("  --pull={pull_value} \\\n"));

    s.push_str("  -v \"$_shclap_script:$_shclap_script:ro\" \\\n");
    s.push_str("  -v \"$_shclap_bin:/usr/local/bin/shclap:ro\" \\\n");
    s.push_str("  -e SHCLAP_IN_CONTAINER=1 \\\n");

    // Collect environment variable names to forward
    let mut forwarded_vars: HashSet<String> = HashSet::new();

    // Step 1: Collect prefix-matching environment variables
    let prefix = config.effective_prefix();
    for (name, _) in env::vars() {
        if name.starts_with(prefix) {
            forwarded_vars.insert(name);
        }
    }

    // Step 2: Collect explicit env names from args
    fn collect_env_vars(args: &[ArgConfig], config: &Config, forwarded_vars: &mut HashSet<String>) {
        for arg in args {
            if let Some(var_name) =
                arg.effective_env(config.effective_prefix(), config.schema_version)
            {
                forwarded_vars.insert(var_name);
            }
        }
    }

    collect_env_vars(&config.args, config, &mut forwarded_vars);

    // Also collect from subcommands
    for subcmd in &config.subcommands {
        collect_env_vars(&subcmd.args, config, &mut forwarded_vars);
    }

    // Step 3: Emit the -e lines in sorted order for deterministic output
    let mut sorted_vars: Vec<_> = forwarded_vars.into_iter().collect();
    sorted_vars.sort();
    for var_name in sorted_vars {
        s.push_str(&format!("  -e {var_name} \\\n"));
    }

    // Quoted, not interpolated raw: these values come from the config file and
    // are emitted into a file the caller sources.
    for arg in &container.args {
        s.push_str(&format!("  {} \\\n", shell_quote(arg)));
    }
    s.push_str(&format!("  {} \\\n", shell_quote(&container.image)));
    s.push_str("  bash \"$_shclap_script\" \"$@\"\n");
    s
}

/// Write the container re-exec shell fragment to a temp file and return the path.
pub fn generate_container_reexec_output(
    container: &ContainerConfig,
    config: &Config,
) -> Result<PathBuf> {
    let content = generate_container_reexec_string(container, config);
    write_temp_file(&content)
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

    // Container reexec tests

    #[test]
    fn test_generate_container_reexec_string_no_extra_args() {
        use crate::config::{Config, ContainerConfig};

        let container = ContainerConfig {
            runtime: "docker".to_string(),
            image: "ubuntu:22.04".to_string(),
            args: vec![],
        };
        let config = Config::from_json(r#"{"schema_version": 2, "name": "test"}"#).unwrap();

        let output = generate_container_reexec_string(&container, &config);

        // Must start with canonical script / binary path resolution
        assert!(output.contains("_shclap_script=$(readlink -f \"$0\")"));
        assert!(output.contains("_shclap_bin=$(readlink -f \"$(command -v shclap)\")"));
        // Runtime availability check with correct error message
        assert!(output.contains("command -v docker >/dev/null 2>&1 || { echo \"shclap: container runtime 'docker' not found\" >&2; exit 127; }"));
        // exec form with script and binary volume mounts
        assert!(output.contains("exec docker run --rm \\"));
        assert!(output.contains("-v \"$_shclap_script:$_shclap_script:ro\" \\"));
        assert!(output.contains("-v \"$_shclap_bin:/usr/local/bin/shclap:ro\" \\"));
        assert!(output.contains("-e SHCLAP_IN_CONTAINER=1 \\"));
        // image and re-exec
        assert!(output.contains("ubuntu:22.04 \\"));
        assert!(output.contains("bash \"$_shclap_script\" \"$@\""));
    }

    #[test]
    fn test_generate_container_reexec_string_with_extra_args() {
        use crate::config::{Config, ContainerConfig};

        let container = ContainerConfig {
            runtime: "podman".to_string(),
            image: "fedora:39".to_string(),
            args: vec!["-v".to_string(), "/host:/container:ro".to_string()],
        };
        let config = Config::from_json(r#"{"schema_version": 2, "name": "test"}"#).unwrap();

        let output = generate_container_reexec_string(&container, &config);

        // Extra args must appear verbatim before the image
        assert!(output.contains("-v \\"));
        assert!(output.contains("/host:/container:ro \\"));
        // Image comes after extra args
        let v_pos = output.find("-v \\\n  /host:/container:ro \\").unwrap();
        let img_pos = output.find("fedora:39 \\").unwrap();
        assert!(v_pos < img_pos);
    }

    #[test]
    fn test_container_reexec_quotes_args_containing_spaces() {
        use crate::config::{Config, ContainerConfig};

        let container = ContainerConfig {
            runtime: "docker".to_string(),
            image: "ubuntu:24.04".to_string(),
            args: vec!["--label".to_string(), "my label".to_string()],
        };
        let config = Config::from_json(r#"{"schema_version": 2, "name": "test"}"#).unwrap();

        let output = generate_container_reexec_string(&container, &config);

        // Unquoted, the shell would split this into two arguments.
        assert!(output.contains("  'my label' \\\n"));
        assert!(!output.contains("  my label \\\n"));
    }

    #[test]
    fn test_container_reexec_quotes_args_with_shell_metacharacters() {
        use crate::config::{Config, ContainerConfig};

        let container = ContainerConfig {
            runtime: "docker".to_string(),
            image: "ubuntu:24.04".to_string(),
            args: vec![
                "--label=a;id".to_string(),
                "$(id)".to_string(),
                "*".to_string(),
            ],
        };
        let config = Config::from_json(r#"{"schema_version": 2, "name": "test"}"#).unwrap();

        let output = generate_container_reexec_string(&container, &config);

        // The generated file is sourced, so none of these may reach the shell bare.
        assert!(output.contains("  '--label=a;id' \\\n"));
        assert!(output.contains("  '$(id)' \\\n"));
        assert!(output.contains("  '*' \\\n"));
    }

    #[test]
    fn test_container_reexec_quotes_single_quote_in_arg() {
        use crate::config::{Config, ContainerConfig};

        let container = ContainerConfig {
            runtime: "docker".to_string(),
            image: "ubuntu:24.04".to_string(),
            args: vec!["it's".to_string()],
        };
        let config = Config::from_json(r#"{"schema_version": 2, "name": "test"}"#).unwrap();

        let output = generate_container_reexec_string(&container, &config);

        assert!(output.contains(r"  'it'\''s' \"));
    }

    #[test]
    fn test_container_reexec_leaves_ordinary_args_unquoted() {
        use crate::config::{Config, ContainerConfig};

        let container = ContainerConfig {
            runtime: "podman".to_string(),
            image: "registry.example.com/team/img:v1.2.3".to_string(),
            args: vec![
                "--network".to_string(),
                "host".to_string(),
                "-v".to_string(),
                "/host:/container:ro".to_string(),
            ],
        };
        let config = Config::from_json(r#"{"schema_version": 2, "name": "test"}"#).unwrap();

        let output = generate_container_reexec_string(&container, &config);

        // Values that are inert to the shell stay readable.
        assert!(output.contains("  --network \\\n  host \\\n"));
        assert!(output.contains("  /host:/container:ro \\\n"));
        assert!(output.contains("  registry.example.com/team/img:v1.2.3 \\\n"));
    }

    #[test]
    fn test_container_reexec_quotes_image_with_metacharacters() {
        use crate::config::{Config, ContainerConfig};

        let container = ContainerConfig {
            runtime: "docker".to_string(),
            image: "ubuntu:24.04 ; id".to_string(),
            args: vec![],
        };
        let config = Config::from_json(r#"{"schema_version": 2, "name": "test"}"#).unwrap();

        let output = generate_container_reexec_string(&container, &config);

        assert!(output.contains("  'ubuntu:24.04 ; id' \\\n"));
    }

    #[test]
    fn test_shell_quote_allowlist() {
        // Inert values pass through bare.
        assert_eq!(shell_quote("ubuntu:24.04"), "ubuntu:24.04");
        assert_eq!(shell_quote("--network"), "--network");
        assert_eq!(shell_quote("/host:/container:ro"), "/host:/container:ro");
        assert_eq!(shell_quote("a,b=c@d%e+f"), "a,b=c@d%e+f");

        // Everything else is quoted.
        assert_eq!(shell_quote(""), "''");
        assert_eq!(shell_quote("a b"), "'a b'");
        assert_eq!(shell_quote("a;id"), "'a;id'");
        assert_eq!(shell_quote("a|b"), "'a|b'");
        assert_eq!(shell_quote("a&b"), "'a&b'");
        assert_eq!(shell_quote("$(id)"), "'$(id)'");
        assert_eq!(shell_quote("`id`"), "'`id`'");
        assert_eq!(shell_quote("a>b"), "'a>b'");
        assert_eq!(shell_quote("*"), "'*'");
        assert_eq!(shell_quote("~root"), "'~root'");
        assert_eq!(shell_quote("a\nb"), "'a\nb'");
        assert_eq!(shell_quote("it's"), r"'it'\''s'");
    }

    #[test]
    fn test_generate_container_reexec_output_creates_file() {
        use crate::config::{Config, ContainerConfig};

        let container = ContainerConfig {
            runtime: "docker".to_string(),
            image: "alpine:3".to_string(),
            args: vec![],
        };
        let config = Config::from_json(r#"{"schema_version": 2, "name": "test"}"#).unwrap();

        let path = generate_container_reexec_output(&container, &config).unwrap();
        assert!(path.exists());
        let contents = std::fs::read_to_string(&path).unwrap();
        assert!(contents.contains("exec docker run --rm"));
        assert!(contents.contains("alpine:3"));
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn test_container_reexec_forwards_prefix_matching_env_vars() {
        use crate::config::{Config, ContainerConfig};

        let container = ContainerConfig {
            runtime: "docker".to_string(),
            image: "ubuntu:22.04".to_string(),
            args: vec![],
        };
        let config =
            Config::from_json(r#"{"schema_version": 2, "name": "test", "prefix": "MYAPP_"}"#)
                .unwrap();

        // Set environment variables with the prefix
        env::set_var("MYAPP_DEBUG", "true");
        env::set_var("MYAPP_LOG_LEVEL", "info");
        env::set_var("UNRELATED_VAR", "should_not_appear");

        let output = generate_container_reexec_string(&container, &config);

        // Clean up
        env::remove_var("MYAPP_DEBUG");
        env::remove_var("MYAPP_LOG_LEVEL");
        env::remove_var("UNRELATED_VAR");

        // Should forward prefix-matching vars
        assert!(output.contains("-e MYAPP_DEBUG \\"));
        assert!(output.contains("-e MYAPP_LOG_LEVEL \\"));
        // Should not forward unrelated vars
        assert!(!output.contains("UNRELATED_VAR"));
    }

    #[test]
    fn test_container_reexec_forwards_custom_env_names() {
        use crate::config::{Config, ContainerConfig};

        let container = ContainerConfig {
            runtime: "docker".to_string(),
            image: "ubuntu:22.04".to_string(),
            args: vec![],
        };

        let config_json = r#"{
            "schema_version": 2,
            "name": "test",
            "args": [
                {
                    "name": "verbose",
                    "type": "flag",
                    "env": "CUSTOM_VERBOSE"
                },
                {
                    "name": "config",
                    "type": "option",
                    "env": "CONFIG_PATH"
                }
            ]
        }"#;

        let config = Config::from_json(config_json).unwrap();

        let output = generate_container_reexec_string(&container, &config);

        // Should forward custom env names
        assert!(output.contains("-e CUSTOM_VERBOSE \\"));
        assert!(output.contains("-e CONFIG_PATH \\"));
    }

    #[test]
    fn test_container_reexec_deduplicates_env_vars() {
        use crate::config::{Config, ContainerConfig};

        let container = ContainerConfig {
            runtime: "docker".to_string(),
            image: "ubuntu:22.04".to_string(),
            args: vec![],
        };

        let config_json = r#"{
            "schema_version": 2,
            "name": "test",
            "prefix": "SHCLAP_",
            "args": [
                {
                    "name": "verbose",
                    "type": "flag",
                    "env": "SHCLAP_VERBOSE"
                }
            ]
        }"#;

        let config = Config::from_json(config_json).unwrap();

        // Set an env var that matches both the prefix and a custom env name
        env::set_var("SHCLAP_VERBOSE", "true");

        let output = generate_container_reexec_string(&container, &config);

        // Clean up
        env::remove_var("SHCLAP_VERBOSE");

        // Should appear only once, not duplicated
        let count = output.matches("-e SHCLAP_VERBOSE \\").count();
        assert_eq!(count, 1, "SHCLAP_VERBOSE should appear exactly once");
    }

    // Tests for --pull flag support
    #[test]
    fn test_container_reexec_pull_policy_always_docker() {
        use crate::config::{Config, ContainerConfig, PullPolicy};

        let container = ContainerConfig {
            runtime: "docker".to_string(),
            image: "ubuntu:22.04".to_string(),
            args: vec![],
        };
        let config_json = r#"{"schema_version": 2, "name": "test", "pull_policy": "always"}"#;
        let config = Config::from_json(config_json).unwrap();

        let output = generate_container_reexec_string(&container, &config);

        // Should contain --pull=always immediately after --rm
        assert!(output.contains("exec docker run --rm \\\n  --pull=always \\"));
    }

    #[test]
    fn test_container_reexec_pull_policy_never_docker() {
        use crate::config::{Config, ContainerConfig, PullPolicy};

        let container = ContainerConfig {
            runtime: "docker".to_string(),
            image: "ubuntu:22.04".to_string(),
            args: vec![],
        };
        let config_json = r#"{"schema_version": 2, "name": "test", "pull_policy": "never"}"#;
        let config = Config::from_json(config_json).unwrap();

        let output = generate_container_reexec_string(&container, &config);

        // Should contain --pull=never immediately after --rm
        assert!(output.contains("exec docker run --rm \\\n  --pull=never \\"));
    }

    #[test]
    fn test_container_reexec_pull_policy_ifnotpresent_docker() {
        use crate::config::{Config, ContainerConfig, PullPolicy};

        let container = ContainerConfig {
            runtime: "docker".to_string(),
            image: "ubuntu:22.04".to_string(),
            args: vec![],
        };
        let config_json = r#"{"schema_version": 2, "name": "test", "pull_policy": "ifnotpresent"}"#;
        let config = Config::from_json(config_json).unwrap();

        let output = generate_container_reexec_string(&container, &config);

        // Should contain --pull=missing immediately after --rm
        assert!(output.contains("exec docker run --rm \\\n  --pull=missing \\"));
    }

    #[test]
    fn test_container_reexec_pull_policy_always_podman() {
        use crate::config::{Config, ContainerConfig, PullPolicy};

        let container = ContainerConfig {
            runtime: "podman".to_string(),
            image: "fedora:39".to_string(),
            args: vec![],
        };
        let config_json = r#"{"schema_version": 2, "name": "test", "pull_policy": "always"}"#;
        let config = Config::from_json(config_json).unwrap();

        let output = generate_container_reexec_string(&container, &config);

        // Should contain --pull=always immediately after --rm
        assert!(output.contains("exec podman run --rm \\\n  --pull=always \\"));
    }

    #[test]
    fn test_container_reexec_pull_policy_never_podman() {
        use crate::config::{Config, ContainerConfig, PullPolicy};

        let container = ContainerConfig {
            runtime: "podman".to_string(),
            image: "fedora:39".to_string(),
            args: vec![],
        };
        let config_json = r#"{"schema_version": 2, "name": "test", "pull_policy": "never"}"#;
        let config = Config::from_json(config_json).unwrap();

        let output = generate_container_reexec_string(&container, &config);

        // Should contain --pull=never immediately after --rm
        assert!(output.contains("exec podman run --rm \\\n  --pull=never \\"));
    }

    #[test]
    fn test_container_reexec_pull_policy_ifnotpresent_podman() {
        use crate::config::{Config, ContainerConfig, PullPolicy};

        let container = ContainerConfig {
            runtime: "podman".to_string(),
            image: "fedora:39".to_string(),
            args: vec![],
        };
        let config_json = r#"{"schema_version": 2, "name": "test", "pull_policy": "ifnotpresent"}"#;
        let config = Config::from_json(config_json).unwrap();

        let output = generate_container_reexec_string(&container, &config);

        // Should contain --pull=missing immediately after --rm
        assert!(output.contains("exec podman run --rm \\\n  --pull=missing \\"));
    }

    #[test]
    fn test_container_reexec_default_pull_policy_is_ifnotpresent() {
        use crate::config::{Config, ContainerConfig};

        let container = ContainerConfig {
            runtime: "docker".to_string(),
            image: "alpine:3".to_string(),
            args: vec![],
        };
        // No pull_policy specified, should default to IfNotPresent
        let config_json = r#"{"schema_version": 2, "name": "test"}"#;
        let config = Config::from_json(config_json).unwrap();

        let output = generate_container_reexec_string(&container, &config);

        // Should contain --pull=missing (the mapped value for IfNotPresent) immediately after --rm
        assert!(output.contains("exec docker run --rm \\\n  --pull=missing \\"));
    }
}
