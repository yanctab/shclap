//! Temporary file generation with shell export statements and special outputs.

use crate::config::{ArgConfig, ArgType, Config, ContainerConfig};
use crate::parser::ParsedValue;
use anyhow::{Context, Result};
use std::collections::HashMap;
use std::env;
use std::io::Write;
use std::path::{Path, PathBuf};
use tempfile::NamedTempFile;
use uzers;

/// Heredoc delimiter for help output.
const HELP_DELIMITER: &str = "SHCLAP_HELP";
/// Heredoc delimiter for version output.
const VERSION_DELIMITER: &str = "SHCLAP_VERSION";

/// Host identity information (UID and GID).
/// Used as a test seam for container execution identity configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HostIdentity {
    /// User ID (UID)
    pub uid: u32,
    /// Group ID (GID)
    pub gid: u32,
}

impl HostIdentity {
    /// Get the current process's user and group IDs.
    pub fn current() -> Self {
        Self {
            uid: uzers::get_current_uid(),
            gid: uzers::get_current_gid(),
        }
    }
}

/// Shell code for log helper functions.
/// These five functions are defined when sourcing parse output,
/// allowing shell scripts to use log_error, log_warn, log_info, log_debug, log_trace.
/// Each helper delegates to `shclap log <level>` and respects `SHCLAP_LOG`.
const LOG_HELPERS: &str = r#"log_error() { shclap log error "$@"; }
log_warn() { shclap log warn "$@"; }
log_info() { shclap log info "$@"; }
log_debug() { shclap log debug "$@"; }
log_trace() { shclap log trace "$@"; }
"#;

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

    // Prepend log helper functions
    output.push_str(LOG_HELPERS);

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

    // Prepend log helper functions
    output.push_str(LOG_HELPERS);

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
pub fn generate_container_reexec_string(
    container: &ContainerConfig,
    config: &Config,
    script: &Path,
) -> Result<String> {
    generate_container_reexec_string_with(
        || {
            std::env::current_dir()
                .and_then(std::fs::canonicalize)
                .map_err(|e| anyhow::anyhow!(e))
        },
        || std::env::current_exe().map_err(|e| anyhow::anyhow!(e)),
        || Ok(script.to_path_buf()),
        HostIdentity::current(),
        container,
        config,
    )
}

/// Generate the shell fragment that re-execs the calling script inside a container,
/// with injected closures for cwd, exe, and script path resolution.
///
/// All three paths are resolved at parse time and emitted as literals.
/// Returns an error if any path cannot be resolved.
pub(crate) fn generate_container_reexec_string_with<CwdFn, ExeFn, ScriptFn>(
    cwd_fn: CwdFn,
    exe_fn: ExeFn,
    script_fn: ScriptFn,
    identity: HostIdentity,
    container: &ContainerConfig,
    config: &Config,
) -> Result<String>
where
    CwdFn: Fn() -> Result<PathBuf>,
    ExeFn: Fn() -> Result<PathBuf>,
    ScriptFn: Fn() -> Result<PathBuf>,
{
    use std::collections::HashSet;

    let script_path = script_fn().context("failed to resolve script path")?;
    let exe_path = exe_fn().context("failed to resolve shclap binary path")?;
    let cwd_path = cwd_fn().context("failed to resolve current working directory")?;

    let rt = &container.runtime;
    let mut s = String::new();

    s.push_str(&format!(
        "_shclap_script={}\n",
        shell_quote(&script_path.to_string_lossy())
    ));
    s.push_str(&format!(
        "_shclap_bin={}\n",
        shell_quote(&exe_path.to_string_lossy())
    ));
    s.push_str(&format!(
        "command -v {rt} >/dev/null 2>&1 || {{ echo \"shclap: container runtime '{rt}' not found\" >&2; exit 127; }}\n"
    ));
    s.push_str(&format!(
        "echo \"shclap: bootstrapping into {rt}:{image}\" >&2\n",
        image = container.image
    ));
    s.push_str(&format!(
        "_shclap_cwd={}\n",
        shell_quote(&cwd_path.to_string_lossy())
    ));
    s.push_str(&format!("_shclap_uid={}\n", identity.uid));
    s.push_str(&format!("_shclap_gid={}\n", identity.gid));
    s.push_str("set -x\n");
    s.push_str(&format!("exec {rt} run --rm \\\n"));

    // Emit --pull flag — enum Display yields the verbatim runtime value
    s.push_str(&format!("  --pull={} \\\n", container.pull_policy));

    // Emit host user identity flags when enabled
    if container.host_user {
        s.push_str("  -u \"$_shclap_uid:$_shclap_gid\" \\\n");
        s.push_str("  -v /etc/passwd:/etc/passwd:ro \\\n");
        s.push_str("  -v /etc/group:/etc/group:ro \\\n");
    }

    s.push_str("  -v \"$_shclap_script:$_shclap_script:ro\" \\\n");
    s.push_str("  -v \"$_shclap_bin:/usr/local/bin/shclap:ro\" \\\n");
    s.push_str("  -e SHCLAP_IN_CONTAINER=1 \\\n");

    // Forward SHCLAP_LOG and SHCLAP_LOG_STYLE immediately after SHCLAP_IN_CONTAINER=1
    if env::var("SHCLAP_LOG").is_ok() {
        s.push_str("  -e SHCLAP_LOG \\\n");
    }
    if env::var("SHCLAP_LOG_STYLE").is_ok() {
        s.push_str("  -e SHCLAP_LOG_STYLE \\\n");
    }

    s.push_str("  -v \"$_shclap_cwd:$_shclap_cwd\" \\\n");
    s.push_str("  --workdir \"$_shclap_cwd\" \\\n");

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
    // Exclude SHCLAP_LOG and SHCLAP_LOG_STYLE (already emitted above)
    let mut sorted_vars: Vec<_> = forwarded_vars
        .into_iter()
        .filter(|var| var != "SHCLAP_LOG" && var != "SHCLAP_LOG_STYLE")
        .collect();
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
    Ok(s)
}

/// Write the container re-exec shell fragment to a temp file and return the path.
pub fn generate_container_reexec_output(
    container: &ContainerConfig,
    config: &Config,
    script: &Path,
) -> Result<PathBuf> {
    let content = generate_container_reexec_string(container, config, script)?;
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
#[allow(clippy::needless_update)]
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
            ..Default::default()
        };
        let config = Config::from_json(r#"{"schema_version": 2, "name": "test"}"#).unwrap();

        let output = generate_container_reexec_string(
            &container,
            &config,
            std::path::Path::new("/test/script.sh"),
        )
        .unwrap();

        // All paths are emitted as literals resolved at parse time
        assert!(output.contains("_shclap_script=/test/script.sh"));
        assert!(output.contains("_shclap_bin="));
        assert!(!output.contains("_shclap_bin=$(readlink -f \"$(command -v shclap)\")"));
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
            ..Default::default()
        };
        let config = Config::from_json(r#"{"schema_version": 2, "name": "test"}"#).unwrap();

        let output = generate_container_reexec_string(
            &container,
            &config,
            std::path::Path::new("/test/script.sh"),
        )
        .unwrap();

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
            ..Default::default()
        };
        let config = Config::from_json(r#"{"schema_version": 2, "name": "test"}"#).unwrap();

        let output = generate_container_reexec_string(
            &container,
            &config,
            std::path::Path::new("/test/script.sh"),
        )
        .unwrap();

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
            ..Default::default()
        };
        let config = Config::from_json(r#"{"schema_version": 2, "name": "test"}"#).unwrap();

        let output = generate_container_reexec_string(
            &container,
            &config,
            std::path::Path::new("/test/script.sh"),
        )
        .unwrap();

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
            ..Default::default()
        };
        let config = Config::from_json(r#"{"schema_version": 2, "name": "test"}"#).unwrap();

        let output = generate_container_reexec_string(
            &container,
            &config,
            std::path::Path::new("/test/script.sh"),
        )
        .unwrap();

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
            ..Default::default()
        };
        let config = Config::from_json(r#"{"schema_version": 2, "name": "test"}"#).unwrap();

        let output = generate_container_reexec_string(
            &container,
            &config,
            std::path::Path::new("/test/script.sh"),
        )
        .unwrap();

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
            ..Default::default()
        };
        let config = Config::from_json(r#"{"schema_version": 2, "name": "test"}"#).unwrap();

        let output = generate_container_reexec_string(
            &container,
            &config,
            std::path::Path::new("/test/script.sh"),
        )
        .unwrap();

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
            ..Default::default()
        };
        let config = Config::from_json(r#"{"schema_version": 2, "name": "test"}"#).unwrap();

        let path = generate_container_reexec_output(
            &container,
            &config,
            std::path::Path::new("/test/script.sh"),
        )
        .unwrap();
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
            ..Default::default()
        };
        let config =
            Config::from_json(r#"{"schema_version": 2, "name": "test", "prefix": "MYAPP_"}"#)
                .unwrap();

        // Set environment variables with the prefix
        env::set_var("MYAPP_DEBUG", "true");
        env::set_var("MYAPP_LOG_LEVEL", "info");
        env::set_var("UNRELATED_VAR", "should_not_appear");

        let output = generate_container_reexec_string(
            &container,
            &config,
            std::path::Path::new("/test/script.sh"),
        )
        .unwrap();

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
            ..Default::default()
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

        let output = generate_container_reexec_string(
            &container,
            &config,
            std::path::Path::new("/test/script.sh"),
        )
        .unwrap();

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
            ..Default::default()
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

        let output = generate_container_reexec_string(
            &container,
            &config,
            std::path::Path::new("/test/script.sh"),
        )
        .unwrap();

        // Clean up
        env::remove_var("SHCLAP_VERBOSE");

        // Should appear only once, not duplicated
        let count = output.matches("-e SHCLAP_VERBOSE \\").count();
        assert_eq!(count, 1, "SHCLAP_VERBOSE should appear exactly once");
    }

    // Tests for --pull flag support
    #[test]
    fn test_container_reexec_pull_policy_always_docker() {
        use crate::config::Config;

        let config = Config::from_json(
            r#"{"schema_version": 2, "name": "test", "container": {"runtime": "docker", "image": "ubuntu:22.04", "pull_policy": "always"}}"#,
        )
        .unwrap();
        let container = config.container.as_ref().unwrap();

        let output = generate_container_reexec_string(
            container,
            &config,
            std::path::Path::new("/test/script.sh"),
        )
        .unwrap();

        // Should contain --pull=always immediately after --rm
        assert!(output.contains("exec docker run --rm \\\n  --pull=always \\"));
    }

    #[test]
    fn test_container_reexec_pull_policy_never_docker() {
        use crate::config::Config;

        let config = Config::from_json(
            r#"{"schema_version": 2, "name": "test", "container": {"runtime": "docker", "image": "ubuntu:22.04", "pull_policy": "never"}}"#,
        )
        .unwrap();
        let container = config.container.as_ref().unwrap();

        let output = generate_container_reexec_string(
            container,
            &config,
            std::path::Path::new("/test/script.sh"),
        )
        .unwrap();

        // Should contain --pull=never immediately after --rm
        assert!(output.contains("exec docker run --rm \\\n  --pull=never \\"));
    }

    #[test]
    fn test_container_reexec_pull_policy_missing_docker() {
        use crate::config::Config;

        let config = Config::from_json(
            r#"{"schema_version": 2, "name": "test", "container": {"runtime": "docker", "image": "ubuntu:22.04", "pull_policy": "missing"}}"#,
        )
        .unwrap();
        let container = config.container.as_ref().unwrap();

        let output = generate_container_reexec_string(
            container,
            &config,
            std::path::Path::new("/test/script.sh"),
        )
        .unwrap();

        // Should contain --pull=missing immediately after --rm
        assert!(output.contains("exec docker run --rm \\\n  --pull=missing \\"));
    }

    #[test]
    fn test_container_reexec_pull_policy_always_podman() {
        use crate::config::Config;

        let config = Config::from_json(
            r#"{"schema_version": 2, "name": "test", "container": {"runtime": "podman", "image": "fedora:39", "pull_policy": "always"}}"#,
        )
        .unwrap();
        let container = config.container.as_ref().unwrap();

        let output = generate_container_reexec_string(
            container,
            &config,
            std::path::Path::new("/test/script.sh"),
        )
        .unwrap();

        // Should contain --pull=always immediately after --rm
        assert!(output.contains("exec podman run --rm \\\n  --pull=always \\"));
    }

    #[test]
    fn test_container_reexec_pull_policy_never_podman() {
        use crate::config::Config;

        let config = Config::from_json(
            r#"{"schema_version": 2, "name": "test", "container": {"runtime": "podman", "image": "fedora:39", "pull_policy": "never"}}"#,
        )
        .unwrap();
        let container = config.container.as_ref().unwrap();

        let output = generate_container_reexec_string(
            container,
            &config,
            std::path::Path::new("/test/script.sh"),
        )
        .unwrap();

        // Should contain --pull=never immediately after --rm
        assert!(output.contains("exec podman run --rm \\\n  --pull=never \\"));
    }

    #[test]
    fn test_container_reexec_pull_policy_missing_podman() {
        use crate::config::Config;

        let config = Config::from_json(
            r#"{"schema_version": 2, "name": "test", "container": {"runtime": "podman", "image": "fedora:39", "pull_policy": "missing"}}"#,
        )
        .unwrap();
        let container = config.container.as_ref().unwrap();

        let output = generate_container_reexec_string(
            container,
            &config,
            std::path::Path::new("/test/script.sh"),
        )
        .unwrap();

        // Should contain --pull=missing immediately after --rm
        assert!(output.contains("exec podman run --rm \\\n  --pull=missing \\"));
    }

    #[test]
    fn test_container_reexec_default_pull_policy_is_missing() {
        use crate::config::Config;

        // No pull_policy specified in container — should default to Missing
        let config = Config::from_json(
            r#"{"schema_version": 2, "name": "test", "container": {"runtime": "docker", "image": "alpine:3"}}"#,
        )
        .unwrap();
        let container = config.container.as_ref().unwrap();

        let output = generate_container_reexec_string(
            container,
            &config,
            std::path::Path::new("/test/script.sh"),
        )
        .unwrap();

        // Should contain --pull=missing (the verbatim string for PullPolicy::Missing) immediately after --rm
        assert!(output.contains("exec docker run --rm \\\n  --pull=missing \\"));
    }

    #[test]
    fn test_generate_container_reexec_emits_cwd_mount_and_workdir() {
        use crate::config::{Config, ContainerConfig};

        let container = ContainerConfig {
            runtime: "docker".to_string(),
            image: "ubuntu:22.04".to_string(),
            args: vec!["-v".to_string(), "/data:/data".to_string()],
            ..Default::default()
        };
        let config = Config::from_json(r#"{"schema_version": 2, "name": "test"}"#).unwrap();

        let output = generate_container_reexec_string(
            &container,
            &config,
            std::path::Path::new("/test/script.sh"),
        )
        .unwrap();

        // Check that _shclap_cwd (literal value) appears before exec line (new form)
        assert!(output.contains("_shclap_cwd="));
        assert!(!output.contains("_shclap_cwd=$(pwd)"));
        assert!(!output.contains("_shclap_cwd=$(pwd -P)"));
        let cwd_pos = output.find("_shclap_cwd=").unwrap();
        let exec_pos = output.find("exec docker run").unwrap();
        assert!(
            cwd_pos < exec_pos,
            "_shclap_cwd must appear before exec line"
        );

        // Check that CWD volume mount appears
        assert!(output.contains("-v \"$_shclap_cwd:$_shclap_cwd\" \\"));

        // Check that CWD volume mount has no :ro suffix
        assert!(!output.contains("-v \"$_shclap_cwd:$_shclap_cwd:ro\""));

        // Check that --workdir line appears
        assert!(output.contains("--workdir \"$_shclap_cwd\" \\"));

        // Check ordering: SHCLAP_IN_CONTAINER before CWD volume
        let container_env_pos = output.find("-e SHCLAP_IN_CONTAINER=1").unwrap();
        let vol_pos = output.find("-v \"$_shclap_cwd:$_shclap_cwd\" \\").unwrap();
        assert!(
            container_env_pos < vol_pos,
            "SHCLAP_IN_CONTAINER must come before CWD volume"
        );

        // Check ordering: CWD volume before workdir
        let workdir_pos = output.find("--workdir \"$_shclap_cwd\" \\").unwrap();
        assert!(vol_pos < workdir_pos, "CWD volume must come before workdir");

        // Check ordering: workdir before container.args
        let arg_pos = output.find("/data:/data").unwrap();
        assert!(
            workdir_pos < arg_pos,
            "workdir must come before container.args"
        );
    }

    #[test]
    fn test_container_reexec_forwards_shclap_log_vars_immediately_after_in_container() {
        use crate::config::{Config, ContainerConfig};

        let container = ContainerConfig {
            runtime: "docker".to_string(),
            image: "ubuntu:22.04".to_string(),
            args: vec![],
            ..Default::default()
        };
        let config = Config::from_json(r#"{"schema_version": 2, "name": "test"}"#).unwrap();

        // Set the SHCLAP_LOG and SHCLAP_LOG_STYLE environment variables
        env::set_var("SHCLAP_LOG", "debug");
        env::set_var("SHCLAP_LOG_STYLE", "always");

        let output = generate_container_reexec_string(
            &container,
            &config,
            std::path::Path::new("/test/script.sh"),
        )
        .unwrap();

        // Clean up
        env::remove_var("SHCLAP_LOG");
        env::remove_var("SHCLAP_LOG_STYLE");

        // Verify both variables appear in output
        assert!(
            output.contains("-e SHCLAP_LOG \\"),
            "SHCLAP_LOG should be forwarded"
        );
        assert!(
            output.contains("-e SHCLAP_LOG_STYLE \\"),
            "SHCLAP_LOG_STYLE should be forwarded"
        );

        // Verify ordering: both appear immediately after SHCLAP_IN_CONTAINER=1
        let container_env_pos = output.find("-e SHCLAP_IN_CONTAINER=1").unwrap();
        let log_pos = output.find("-e SHCLAP_LOG \\").unwrap();
        let log_style_pos = output.find("-e SHCLAP_LOG_STYLE \\").unwrap();

        assert!(
            container_env_pos < log_pos,
            "SHCLAP_IN_CONTAINER=1 must come before SHCLAP_LOG"
        );
        assert!(
            log_pos < log_style_pos,
            "SHCLAP_LOG must come before SHCLAP_LOG_STYLE"
        );

        // Verify they come immediately after SHCLAP_IN_CONTAINER=1 (before any volume mounts or workdir)
        let vol_pos = output.find("-v \"$_shclap_cwd:$_shclap_cwd\"").unwrap();
        assert!(
            log_style_pos < vol_pos,
            "SHCLAP_LOG_STYLE must come before volume mounts"
        );
    }

    // Tests for generate_container_reexec_string_with seam

    #[test]
    fn test_generate_container_reexec_string_with_basic() {
        use crate::config::{Config, ContainerConfig};
        use std::path::PathBuf;

        let container = ContainerConfig {
            runtime: "docker".to_string(),
            image: "ubuntu:22.04".to_string(),
            args: vec![],
            ..Default::default()
        };
        let config = Config::from_json(r#"{"schema_version": 2, "name": "test"}"#).unwrap();

        // Inject synthetic paths
        let cwd_fn = || -> Result<PathBuf> { Ok(PathBuf::from("/home/user/project")) };
        let exe_fn = || -> Result<PathBuf> { Ok(PathBuf::from("/usr/local/bin/shclap")) };
        let script_fn = || -> Result<PathBuf> { Ok(PathBuf::from("/home/user/project/script.sh")) };

        let output = generate_container_reexec_string_with(
            cwd_fn,
            exe_fn,
            script_fn,
            HostIdentity {
                uid: 1000,
                gid: 1000,
            },
            &container,
            &config,
        )
        .expect("should succeed");

        // All paths emitted as literals
        assert!(output.contains("_shclap_script=/home/user/project/script.sh"));
        assert!(output.contains("_shclap_bin=/usr/local/bin/shclap"));
        assert!(output.contains("_shclap_cwd=/home/user/project"));
    }

    #[test]
    fn test_generate_container_reexec_string_with_paths_with_spaces() {
        use crate::config::{Config, ContainerConfig};
        use std::path::PathBuf;

        let container = ContainerConfig {
            runtime: "docker".to_string(),
            image: "ubuntu:22.04".to_string(),
            args: vec![],
            ..Default::default()
        };
        let config = Config::from_json(r#"{"schema_version": 2, "name": "test"}"#).unwrap();

        // Inject paths with spaces
        let cwd_fn = || -> Result<PathBuf> { Ok(PathBuf::from("/home/user/my project")) };
        let exe_fn = || -> Result<PathBuf> { Ok(PathBuf::from("/usr/bin/shclap 1.0")) };
        let script_fn =
            || -> Result<PathBuf> { Ok(PathBuf::from("/home/user/my project/script.sh")) };

        let output = generate_container_reexec_string_with(
            cwd_fn,
            exe_fn,
            script_fn,
            HostIdentity {
                uid: 1000,
                gid: 1000,
            },
            &container,
            &config,
        )
        .expect("should succeed");

        // All literal; paths with spaces must be quoted
        assert!(output.contains("_shclap_script='/home/user/my project/script.sh'"));
        assert!(output.contains("_shclap_bin='/usr/bin/shclap 1.0'"));
        assert!(output.contains("_shclap_cwd='/home/user/my project'"));
    }

    #[test]
    fn test_generate_container_reexec_string_with_paths_with_special_chars() {
        use crate::config::{Config, ContainerConfig};
        use std::path::PathBuf;

        let container = ContainerConfig {
            runtime: "docker".to_string(),
            image: "ubuntu:22.04".to_string(),
            args: vec![],
            ..Default::default()
        };
        let config = Config::from_json(r#"{"schema_version": 2, "name": "test"}"#).unwrap();

        // Inject paths with special shell characters
        let cwd_fn = || -> Result<PathBuf> { Ok(PathBuf::from("/home/user/$(whoami)")) };
        let exe_fn = || -> Result<PathBuf> { Ok(PathBuf::from("/usr/bin/shclap;id")) };
        let script_fn = || -> Result<PathBuf> { Ok(PathBuf::from("/home/user/$HOME/script.sh")) };

        let output = generate_container_reexec_string_with(
            cwd_fn,
            exe_fn,
            script_fn,
            HostIdentity {
                uid: 1000,
                gid: 1000,
            },
            &container,
            &config,
        )
        .expect("should succeed");

        // All literal; paths with special characters must be single-quoted
        assert!(output.contains("_shclap_script='"));
        assert!(output.contains("_shclap_bin='"));
        assert!(output.contains("_shclap_cwd='"));
        // All assignments must come before exec
        let cwd_assign_pos = output.find("_shclap_cwd=").unwrap();
        let script_assign_pos = output.find("_shclap_script=").unwrap();
        let bin_assign_pos = output.find("_shclap_bin=").unwrap();
        let exec_pos = output.find("exec docker run").unwrap();
        assert!(cwd_assign_pos < exec_pos);
        assert!(script_assign_pos < exec_pos);
        assert!(bin_assign_pos < exec_pos);
    }

    #[test]
    fn test_generate_container_reexec_string_with_cwd_fn_error() {
        use crate::config::{Config, ContainerConfig};
        use std::path::PathBuf;

        let container = ContainerConfig {
            runtime: "docker".to_string(),
            image: "ubuntu:22.04".to_string(),
            args: vec![],
            ..Default::default()
        };
        let config = Config::from_json(r#"{"schema_version": 2, "name": "test"}"#).unwrap();

        // Inject a failing cwd_fn
        let cwd_fn = || -> Result<PathBuf> { Err(anyhow::anyhow!("cannot get current directory")) };
        let exe_fn = || -> Result<PathBuf> { Ok(PathBuf::from("/usr/local/bin/shclap")) };
        let script_fn = || -> Result<PathBuf> { Ok(PathBuf::from("/home/user/script.sh")) };

        let result = generate_container_reexec_string_with(
            cwd_fn,
            exe_fn,
            script_fn,
            HostIdentity {
                uid: 1000,
                gid: 1000,
            },
            &container,
            &config,
        );

        assert!(result.is_err());
    }

    #[test]
    fn test_generate_container_reexec_string_with_exe_fn_error() {
        use crate::config::{Config, ContainerConfig};
        use std::path::PathBuf;

        let container = ContainerConfig {
            runtime: "docker".to_string(),
            image: "ubuntu:22.04".to_string(),
            args: vec![],
            ..Default::default()
        };
        let config = Config::from_json(r#"{"schema_version": 2, "name": "test"}"#).unwrap();

        // Inject a failing exe_fn
        let cwd_fn = || -> Result<PathBuf> { Ok(PathBuf::from("/home/user")) };
        let exe_fn = || -> Result<PathBuf> { Err(anyhow::anyhow!("cannot determine exe path")) };
        let script_fn = || -> Result<PathBuf> { Ok(PathBuf::from("/home/user/script.sh")) };

        let result = generate_container_reexec_string_with(
            cwd_fn,
            exe_fn,
            script_fn,
            HostIdentity {
                uid: 1000,
                gid: 1000,
            },
            &container,
            &config,
        );

        assert!(result.is_err());
    }

    #[test]
    fn test_generate_container_reexec_string_with_script_fn_error() {
        use crate::config::{Config, ContainerConfig};
        use std::path::PathBuf;

        let container = ContainerConfig {
            runtime: "docker".to_string(),
            image: "ubuntu:22.04".to_string(),
            args: vec![],
            ..Default::default()
        };
        let config = Config::from_json(r#"{"schema_version": 2, "name": "test"}"#).unwrap();

        let cwd_fn = || -> Result<PathBuf> { Ok(PathBuf::from("/home/user")) };
        let exe_fn = || -> Result<PathBuf> { Ok(PathBuf::from("/usr/local/bin/shclap")) };
        let script_fn =
            || -> Result<PathBuf> { Err(anyhow::anyhow!("cannot determine script path")) };

        let result = generate_container_reexec_string_with(
            cwd_fn,
            exe_fn,
            script_fn,
            HostIdentity {
                uid: 1000,
                gid: 1000,
            },
            &container,
            &config,
        );

        assert!(result.is_err());
    }

    // --- HostIdentity tests (issue #116) ---

    #[test]
    fn test_host_identity_current() {
        // Criterion 6: HostIdentity::current() calls uzers::get_current_uid() and uzers::get_current_gid()
        let identity = HostIdentity::current();
        // The current process has valid uid/gid
        let actual_uid = uzers::get_current_uid();
        let actual_gid = uzers::get_current_gid();
        assert_eq!(identity.uid, actual_uid);
        assert_eq!(identity.gid, actual_gid);
    }

    // --- New tests for host user flags emission (issue #117) ---

    #[test]
    fn test_container_reexec_emits_host_user_flags_by_default() {
        use crate::config::{Config, ContainerConfig};

        let container = ContainerConfig {
            runtime: "docker".to_string(),
            image: "ubuntu:22.04".to_string(),
            ..Default::default()
        };
        let config = Config::from_json(r#"{"schema_version": 2, "name": "test"}"#).unwrap();
        let identity = HostIdentity {
            uid: 1000,
            gid: 1000,
        };

        let output = generate_container_reexec_string_with(
            || Ok(std::path::PathBuf::from("/home/user")),
            || Ok(std::path::PathBuf::from("/usr/bin/shclap")),
            || Ok(std::path::PathBuf::from("/test/script.sh")),
            identity,
            &container,
            &config,
        )
        .unwrap();

        // When host_user is not specified (defaults to true), flags should be emitted
        assert!(output.contains("_shclap_uid=1000"));
        assert!(output.contains("_shclap_gid=1000"));
        assert!(output.contains("-u \"$_shclap_uid:$_shclap_gid\" \\"));
        assert!(output.contains("-v /etc/passwd:/etc/passwd:ro \\"));
        assert!(output.contains("-v /etc/group:/etc/group:ro \\"));
    }

    #[test]
    fn test_container_reexec_emits_uid_gid_shell_assignments() {
        use crate::config::{Config, ContainerConfig};

        let container = ContainerConfig {
            runtime: "docker".to_string(),
            image: "alpine:latest".to_string(),
            host_user: true,
            ..Default::default()
        };
        let config = Config::from_json(r#"{"schema_version": 2, "name": "test"}"#).unwrap();
        let identity = HostIdentity { uid: 0, gid: 0 };

        let output = generate_container_reexec_string_with(
            || Ok(std::path::PathBuf::from("/")),
            || Ok(std::path::PathBuf::from("/bin/shclap")),
            || Ok(std::path::PathBuf::from("/root/script.sh")),
            identity,
            &container,
            &config,
        )
        .unwrap();

        // Verify uid and gid appear in shell assignments
        assert!(output.contains("_shclap_uid=0"));
        assert!(output.contains("_shclap_gid=0"));
        // Verify the order: uid/gid come after cwd assignment but before set -x
        let cwd_pos = output.find("_shclap_cwd=").unwrap();
        let uid_pos = output.find("_shclap_uid=").unwrap();
        let gid_pos = output.find("_shclap_gid=").unwrap();
        let set_x_pos = output.find("set -x").unwrap();
        assert!(cwd_pos < uid_pos && uid_pos < gid_pos && gid_pos < set_x_pos);
    }

    #[test]
    fn test_container_reexec_omits_host_user_flags_when_disabled() {
        use crate::config::{Config, ContainerConfig};

        let container = ContainerConfig {
            runtime: "docker".to_string(),
            image: "ubuntu:22.04".to_string(),
            host_user: false,
            ..Default::default()
        };
        let config = Config::from_json(r#"{"schema_version": 2, "name": "test"}"#).unwrap();
        let identity = HostIdentity {
            uid: 1000,
            gid: 1000,
        };

        let output = generate_container_reexec_string_with(
            || Ok(std::path::PathBuf::from("/home/user")),
            || Ok(std::path::PathBuf::from("/usr/bin/shclap")),
            || Ok(std::path::PathBuf::from("/test/script.sh")),
            identity,
            &container,
            &config,
        )
        .unwrap();

        // When host_user is false, no user identity flags should appear
        assert!(!output.contains("-u \"$_shclap_uid:$_shclap_gid\""));
        assert!(!output.contains("-v /etc/passwd:/etc/passwd:ro"));
        assert!(!output.contains("-v /etc/group:/etc/group:ro"));
        // But the shell variables should still be emitted (for potential use by the user)
        assert!(output.contains("_shclap_uid=1000"));
        assert!(output.contains("_shclap_gid=1000"));
    }

    #[test]
    fn test_container_reexec_emits_root_when_uid_zero() {
        use crate::config::{Config, ContainerConfig};

        let container = ContainerConfig {
            runtime: "podman".to_string(),
            image: "fedora:39".to_string(),
            host_user: true,
            ..Default::default()
        };
        let config = Config::from_json(r#"{"schema_version": 2, "name": "test"}"#).unwrap();
        let identity = HostIdentity { uid: 0, gid: 0 };

        let output = generate_container_reexec_string_with(
            || Ok(std::path::PathBuf::from("/root")),
            || Ok(std::path::PathBuf::from("/usr/bin/shclap")),
            || Ok(std::path::PathBuf::from("/root/script.sh")),
            identity,
            &container,
            &config,
        )
        .unwrap();

        // Root (uid=0) should still be emitted, no special-casing
        assert!(output.contains("_shclap_uid=0"));
        assert!(output.contains("_shclap_gid=0"));
        assert!(output.contains("-u \"$_shclap_uid:$_shclap_gid\" \\"));
    }

    #[test]
    fn test_container_reexec_container_args_can_override_user() {
        use crate::config::{Config, ContainerConfig};

        let container = ContainerConfig {
            runtime: "docker".to_string(),
            image: "ubuntu:22.04".to_string(),
            args: vec!["--user".to_string(), "9999:9999".to_string()],
            host_user: true,
            ..Default::default()
        };
        let config = Config::from_json(r#"{"schema_version": 2, "name": "test"}"#).unwrap();
        let identity = HostIdentity {
            uid: 1000,
            gid: 1000,
        };

        let output = generate_container_reexec_string_with(
            || Ok(std::path::PathBuf::from("/home/user")),
            || Ok(std::path::PathBuf::from("/usr/bin/shclap")),
            || Ok(std::path::PathBuf::from("/test/script.sh")),
            identity,
            &container,
            &config,
        )
        .unwrap();

        // Shclap-emitted -u should appear before container.args
        let shclap_u_pos = output.find("-u \"$_shclap_uid:$_shclap_gid\" \\").unwrap();
        let container_user_pos = output.find("--user").unwrap();
        assert!(shclap_u_pos < container_user_pos);
        // Docker/podman's last-wins behavior means the container.args user will take precedence
        // but shclap's job is just to emit them in the right order
    }
}
