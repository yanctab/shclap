//! JSON configuration parsing and types for shclap.

use serde::Deserialize;
use thiserror::Error;

/// The minimum supported schema version.
pub const MIN_SCHEMA_VERSION: u32 = 1;
/// The maximum supported schema version.
pub const MAX_SCHEMA_VERSION: u32 = 2;

/// Errors that can occur during config parsing and validation.
#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("failed to parse JSON config: {0}")]
    ParseError(#[from] serde_json::Error),

    #[error("duplicate argument name: {0}")]
    DuplicateName(String),

    #[error("invalid short option '{0}': must be a single ASCII letter")]
    InvalidShortOption(String),

    #[error("argument '{0}' has no short or long option and is not positional")]
    NoOptionSpecified(String),

    #[error("unsupported schema version {0} (supported: 1-2)")]
    UnsupportedSchemaVersion(u32),

    #[error("field '{0}' on argument '{1}' requires schema_version >= 2")]
    FieldRequiresV2(String, String),

    #[error("subcommands require schema_version >= 2")]
    SubcommandsRequireV2,

    #[error("duplicate subcommand name: {0}")]
    DuplicateSubcommandName(String),

    #[error(
        "invalid num_args format '{0}': expected a number or range like '1..', '2..5', or '1..=3'"
    )]
    InvalidNumArgsFormat(String),
}

/// The type of argument.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ArgType {
    /// A boolean flag (e.g., --verbose)
    Flag,
    /// An option that takes a value (e.g., --output file.txt)
    Option,
    /// A positional argument
    Positional,
}

/// Configuration for a single argument.
#[derive(Debug, Clone, Deserialize)]
pub struct ArgConfig {
    /// The name of the argument (used for the environment variable)
    pub name: String,
    /// Short option character (e.g., 'v' for -v)
    pub short: Option<char>,
    /// Long option name (e.g., "verbose" for --verbose)
    pub long: Option<String>,
    /// The type of argument
    #[serde(rename = "type")]
    pub arg_type: ArgType,
    /// Whether this argument is required
    #[serde(default)]
    pub required: bool,
    /// Default value if not provided
    pub default: Option<String>,
    /// Help text for this argument
    pub help: Option<String>,

    // Schema version 2 fields:
    /// Environment variable to use as fallback (schema_version >= 2)
    pub env: Option<String>,
    /// Allow multiple occurrences/values (schema_version >= 2)
    #[serde(default)]
    pub multiple: bool,
    /// Value count range like "1..", "2..5", "1..=3" (schema_version >= 2)
    pub num_args: Option<String>,
    /// Split single value by this delimiter (schema_version >= 2)
    pub delimiter: Option<char>,
}

/// Configuration for a subcommand (schema_version >= 2).
#[derive(Debug, Clone, Deserialize)]
pub struct SubcommandConfig {
    /// The name of the subcommand
    pub name: String,
    /// Help text for this subcommand
    pub help: Option<String>,
    /// Arguments for this subcommand
    #[serde(default)]
    pub args: Vec<ArgConfig>,
}

fn default_schema_version() -> u32 {
    1
}

/// Top-level configuration for a script.
#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    /// Schema version for the config format (default: 1)
    #[serde(default = "default_schema_version")]
    pub schema_version: u32,
    /// Name of the script (optional if provided via CLI --name)
    pub name: Option<String>,
    /// Description of the script
    pub description: Option<String>,
    /// Version of the script
    pub version: Option<String>,
    /// Environment variable prefix (default: "SHCLAP_")
    pub prefix: Option<String>,
    /// List of argument configurations
    #[serde(default)]
    pub args: Vec<ArgConfig>,
    /// Subcommands (schema_version >= 2)
    #[serde(default)]
    pub subcommands: Vec<SubcommandConfig>,
}

impl Config {
    /// Parse a JSON string into a Config.
    pub fn from_json(json: &str) -> Result<Config, ConfigError> {
        let config: Config = serde_json::from_str(json)?;
        Ok(config)
    }

    /// Validate the configuration.
    pub fn validate(&self) -> Result<(), ConfigError> {
        use std::collections::HashSet;

        // Validate schema version
        if self.schema_version < MIN_SCHEMA_VERSION || self.schema_version > MAX_SCHEMA_VERSION {
            return Err(ConfigError::UnsupportedSchemaVersion(self.schema_version));
        }

        // Check for v2 fields when using schema_version 1
        if self.schema_version < 2 {
            if !self.subcommands.is_empty() {
                return Err(ConfigError::SubcommandsRequireV2);
            }
            for arg in &self.args {
                Self::validate_no_v2_fields(arg)?;
            }
        }

        let mut names = HashSet::new();

        for arg in &self.args {
            // Check for duplicate names
            if !names.insert(&arg.name) {
                return Err(ConfigError::DuplicateName(arg.name.clone()));
            }

            Self::validate_arg(arg, self.schema_version)?;
        }

        // Validate subcommands
        if self.schema_version >= 2 {
            let mut subcmd_names = HashSet::new();
            for subcmd in &self.subcommands {
                if !subcmd_names.insert(&subcmd.name) {
                    return Err(ConfigError::DuplicateSubcommandName(subcmd.name.clone()));
                }

                let mut subcmd_arg_names = HashSet::new();
                for arg in &subcmd.args {
                    if !subcmd_arg_names.insert(&arg.name) {
                        return Err(ConfigError::DuplicateName(arg.name.clone()));
                    }
                    Self::validate_arg(arg, self.schema_version)?;
                }
            }
        }

        Ok(())
    }

    /// Validate that an argument doesn't use v2-only fields.
    fn validate_no_v2_fields(arg: &ArgConfig) -> Result<(), ConfigError> {
        if arg.env.is_some() {
            return Err(ConfigError::FieldRequiresV2(
                "env".to_string(),
                arg.name.clone(),
            ));
        }
        if arg.multiple {
            return Err(ConfigError::FieldRequiresV2(
                "multiple".to_string(),
                arg.name.clone(),
            ));
        }
        if arg.num_args.is_some() {
            return Err(ConfigError::FieldRequiresV2(
                "num_args".to_string(),
                arg.name.clone(),
            ));
        }
        if arg.delimiter.is_some() {
            return Err(ConfigError::FieldRequiresV2(
                "delimiter".to_string(),
                arg.name.clone(),
            ));
        }
        Ok(())
    }

    /// Validate a single argument configuration.
    /// Note: This no longer errors when neither short nor long is specified for non-positional args.
    /// Instead, the name will be used as the long option by default.
    fn validate_arg(arg: &ArgConfig, schema_version: u32) -> Result<(), ConfigError> {
        // Validate short option
        if let Some(short) = arg.short {
            if !short.is_ascii_alphabetic() {
                return Err(ConfigError::InvalidShortOption(short.to_string()));
            }
        }

        // Note: We no longer error if neither short nor long is specified.
        // The name will be used as the long option when building the command.

        // Validate num_args format (schema v2)
        if schema_version >= 2 {
            if let Some(ref num_args) = arg.num_args {
                validate_num_args_format(num_args)?;
            }
        }

        Ok(())
    }

    /// Get the effective prefix, using the default if none is set.
    pub fn effective_prefix(&self) -> &str {
        self.prefix.as_deref().unwrap_or("SHCLAP_")
    }
}

/// Validate num_args format (e.g., "1", "1..", "2..5", "1..=3").
fn validate_num_args_format(num_args: &str) -> Result<(), ConfigError> {
    let s = num_args.trim();

    // Single number
    if s.parse::<usize>().is_ok() {
        return Ok(());
    }

    // Range formats: "N..", "N..M", "N..=M"
    if let Some(idx) = s.find("..") {
        let start = &s[..idx];
        let rest = &s[idx + 2..];

        // Start must be a valid number
        if start.parse::<usize>().is_err() {
            return Err(ConfigError::InvalidNumArgsFormat(num_args.to_string()));
        }

        // Rest can be empty (unbounded), a number, or =number
        if rest.is_empty() {
            return Ok(());
        }
        if rest.parse::<usize>().is_ok() {
            return Ok(());
        }
        if let Some(stripped) = rest.strip_prefix('=') {
            if stripped.parse::<usize>().is_ok() {
                return Ok(());
            }
        }
    }

    Err(ConfigError::InvalidNumArgsFormat(num_args.to_string()))
}

impl ArgConfig {
    /// Check if this argument uses any v2-only features.
    pub fn uses_v2_features(&self) -> bool {
        self.env.is_some() || self.multiple || self.num_args.is_some() || self.delimiter.is_some()
    }

    /// Get the effective long option for this argument.
    /// Returns the specified long option, or falls back to the argument name
    /// for non-positional arguments that have no short option.
    pub fn effective_long(&self) -> Option<&str> {
        if self.long.is_some() {
            return self.long.as_deref();
        }
        // For non-positional args without short, use name as long
        if self.arg_type != ArgType::Positional && self.short.is_none() {
            return Some(&self.name);
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_full_config() {
        let json = r#"{
            "name": "myscript",
            "description": "My awesome script",
            "version": "1.0.0",
            "prefix": "MYAPP_",
            "args": [
                {
                    "name": "verbose",
                    "short": "v",
                    "long": "verbose",
                    "type": "flag",
                    "help": "Enable verbose output"
                },
                {
                    "name": "output",
                    "short": "o",
                    "long": "output",
                    "type": "option",
                    "required": true,
                    "help": "Output file"
                },
                {
                    "name": "input",
                    "type": "positional",
                    "required": true,
                    "help": "Input file"
                }
            ]
        }"#;

        let config = Config::from_json(json).unwrap();
        assert_eq!(config.name, Some("myscript".to_string()));
        assert_eq!(config.description, Some("My awesome script".to_string()));
        assert_eq!(config.version, Some("1.0.0".to_string()));
        assert_eq!(config.prefix, Some("MYAPP_".to_string()));
        assert_eq!(config.args.len(), 3);

        // Check verbose arg
        let verbose = &config.args[0];
        assert_eq!(verbose.name, "verbose");
        assert_eq!(verbose.short, Some('v'));
        assert_eq!(verbose.long, Some("verbose".to_string()));
        assert_eq!(verbose.arg_type, ArgType::Flag);
        assert!(!verbose.required);

        // Check output arg
        let output = &config.args[1];
        assert_eq!(output.name, "output");
        assert_eq!(output.arg_type, ArgType::Option);
        assert!(output.required);

        // Check input arg (positional)
        let input = &config.args[2];
        assert_eq!(input.name, "input");
        assert_eq!(input.arg_type, ArgType::Positional);
        assert!(input.required);

        config.validate().unwrap();
    }

    #[test]
    fn test_parse_minimal_config() {
        let json = r#"{"name": "minimal"}"#;
        let config = Config::from_json(json).unwrap();
        assert_eq!(config.name, Some("minimal".to_string()));
        assert!(config.description.is_none());
        assert!(config.version.is_none());
        assert!(config.prefix.is_none());
        assert!(config.args.is_empty());
        config.validate().unwrap();
    }

    #[test]
    fn test_config_without_name_is_valid() {
        // Name is now optional (can be provided via CLI --name)
        let json = r#"{"description": "no name"}"#;
        let config = Config::from_json(json).unwrap();
        assert!(config.name.is_none());
        assert_eq!(config.description, Some("no name".to_string()));
    }

    #[test]
    fn test_error_on_duplicate_arg_names() {
        let json = r#"{
            "name": "test",
            "args": [
                {"name": "dup", "short": "a", "type": "flag"},
                {"name": "dup", "short": "b", "type": "flag"}
            ]
        }"#;
        let config = Config::from_json(json).unwrap();
        let result = config.validate();
        assert!(matches!(result, Err(ConfigError::DuplicateName(name)) if name == "dup"));
    }

    #[test]
    fn test_error_on_invalid_short_option() {
        let json = r#"{
            "name": "test",
            "args": [
                {"name": "bad", "short": "1", "type": "flag"}
            ]
        }"#;
        let config = Config::from_json(json).unwrap();
        let result = config.validate();
        assert!(matches!(result, Err(ConfigError::InvalidShortOption(_))));
    }

    #[test]
    fn test_no_option_specified_uses_name_as_long() {
        // When neither short nor long is specified, name should be used as long
        let json = r#"{
            "name": "test",
            "args": [
                {"name": "verbose", "type": "flag"}
            ]
        }"#;
        let config = Config::from_json(json).unwrap();
        // Validation should pass - name is used as long option
        config.validate().unwrap();
        // Check that effective_long returns the name
        assert_eq!(config.args[0].effective_long(), Some("verbose"));
    }

    #[test]
    fn test_positional_without_short_long_is_valid() {
        let json = r#"{
            "name": "test",
            "args": [
                {"name": "input", "type": "positional"}
            ]
        }"#;
        let config = Config::from_json(json).unwrap();
        config.validate().unwrap();
    }

    #[test]
    fn test_default_value() {
        let json = r#"{
            "name": "test",
            "args": [
                {"name": "output", "long": "output", "type": "option", "default": "out.txt"}
            ]
        }"#;
        let config = Config::from_json(json).unwrap();
        assert_eq!(config.args[0].default, Some("out.txt".to_string()));
    }

    #[test]
    fn test_effective_prefix() {
        let json_with_prefix = r#"{"name": "test", "prefix": "MYAPP_"}"#;
        let config = Config::from_json(json_with_prefix).unwrap();
        assert_eq!(config.effective_prefix(), "MYAPP_");

        let json_without_prefix = r#"{"name": "test"}"#;
        let config = Config::from_json(json_without_prefix).unwrap();
        assert_eq!(config.effective_prefix(), "SHCLAP_");
    }

    #[test]
    fn test_schema_version_defaults_to_1() {
        let json = r#"{"name": "test"}"#;
        let config = Config::from_json(json).unwrap();
        assert_eq!(config.schema_version, 1);
        config.validate().unwrap();
    }

    #[test]
    fn test_schema_version_explicit() {
        let json = r#"{"schema_version": 1, "name": "test"}"#;
        let config = Config::from_json(json).unwrap();
        assert_eq!(config.schema_version, 1);
        config.validate().unwrap();
    }

    #[test]
    fn test_error_on_unsupported_schema_version() {
        let json = r#"{"schema_version": 99, "name": "test"}"#;
        let config = Config::from_json(json).unwrap();
        let result = config.validate();
        assert!(matches!(
            result,
            Err(ConfigError::UnsupportedSchemaVersion(99))
        ));
    }

    // Schema version 2 tests

    #[test]
    fn test_schema_version_2_valid() {
        let json = r#"{"schema_version": 2, "name": "test"}"#;
        let config = Config::from_json(json).unwrap();
        assert_eq!(config.schema_version, 2);
        config.validate().unwrap();
    }

    #[test]
    fn test_schema_v2_env_field() {
        let json = r#"{
            "schema_version": 2,
            "name": "test",
            "args": [
                {"name": "input", "long": "input", "type": "option", "env": "INPUT_FILE"}
            ]
        }"#;
        let config = Config::from_json(json).unwrap();
        config.validate().unwrap();
        assert_eq!(config.args[0].env, Some("INPUT_FILE".to_string()));
    }

    #[test]
    fn test_schema_v2_multiple_field() {
        let json = r#"{
            "schema_version": 2,
            "name": "test",
            "args": [
                {"name": "files", "long": "file", "type": "option", "multiple": true}
            ]
        }"#;
        let config = Config::from_json(json).unwrap();
        config.validate().unwrap();
        assert!(config.args[0].multiple);
    }

    #[test]
    fn test_schema_v2_num_args_field() {
        let json = r#"{
            "schema_version": 2,
            "name": "test",
            "args": [
                {"name": "files", "long": "file", "type": "option", "multiple": true, "num_args": "1.."}
            ]
        }"#;
        let config = Config::from_json(json).unwrap();
        config.validate().unwrap();
        assert_eq!(config.args[0].num_args, Some("1..".to_string()));
    }

    #[test]
    fn test_schema_v2_delimiter_field() {
        let json = r#"{
            "schema_version": 2,
            "name": "test",
            "args": [
                {"name": "tags", "long": "tags", "type": "option", "multiple": true, "delimiter": ","}
            ]
        }"#;
        let config = Config::from_json(json).unwrap();
        config.validate().unwrap();
        assert_eq!(config.args[0].delimiter, Some(','));
    }

    #[test]
    fn test_schema_v2_subcommands() {
        let json = r#"{
            "schema_version": 2,
            "name": "test",
            "subcommands": [
                {
                    "name": "init",
                    "help": "Initialize a project",
                    "args": [
                        {"name": "template", "type": "positional"}
                    ]
                },
                {
                    "name": "run",
                    "help": "Run the project"
                }
            ]
        }"#;
        let config = Config::from_json(json).unwrap();
        config.validate().unwrap();
        assert_eq!(config.subcommands.len(), 2);
        assert_eq!(config.subcommands[0].name, "init");
        assert_eq!(
            config.subcommands[0].help,
            Some("Initialize a project".to_string())
        );
        assert_eq!(config.subcommands[0].args.len(), 1);
        assert_eq!(config.subcommands[1].name, "run");
    }

    #[test]
    fn test_error_v2_field_in_v1_config_env() {
        let json = r#"{
            "schema_version": 1,
            "name": "test",
            "args": [
                {"name": "input", "long": "input", "type": "option", "env": "INPUT_FILE"}
            ]
        }"#;
        let config = Config::from_json(json).unwrap();
        let result = config.validate();
        assert!(matches!(result, Err(ConfigError::FieldRequiresV2(field, _)) if field == "env"));
    }

    #[test]
    fn test_error_v2_field_in_v1_config_multiple() {
        let json = r#"{
            "schema_version": 1,
            "name": "test",
            "args": [
                {"name": "files", "long": "file", "type": "option", "multiple": true}
            ]
        }"#;
        let config = Config::from_json(json).unwrap();
        let result = config.validate();
        assert!(
            matches!(result, Err(ConfigError::FieldRequiresV2(field, _)) if field == "multiple")
        );
    }

    #[test]
    fn test_error_subcommands_in_v1_config() {
        let json = r#"{
            "schema_version": 1,
            "name": "test",
            "subcommands": [{"name": "init"}]
        }"#;
        let config = Config::from_json(json).unwrap();
        let result = config.validate();
        assert!(matches!(result, Err(ConfigError::SubcommandsRequireV2)));
    }

    #[test]
    fn test_error_duplicate_subcommand_name() {
        let json = r#"{
            "schema_version": 2,
            "name": "test",
            "subcommands": [
                {"name": "init"},
                {"name": "init"}
            ]
        }"#;
        let config = Config::from_json(json).unwrap();
        let result = config.validate();
        assert!(
            matches!(result, Err(ConfigError::DuplicateSubcommandName(name)) if name == "init")
        );
    }

    #[test]
    fn test_valid_num_args_formats() {
        let formats = vec![
            "1", "2", "10", "1..", "2..", "1..3", "2..5", "1..=3", "0..=10",
        ];
        for fmt in formats {
            assert!(
                validate_num_args_format(fmt).is_ok(),
                "Expected '{}' to be valid",
                fmt
            );
        }
    }

    #[test]
    fn test_invalid_num_args_formats() {
        let formats = vec!["abc", "..", "..3", "a..b", "1..=", "1..=abc", "-1", "1...3"];
        for fmt in formats {
            assert!(
                validate_num_args_format(fmt).is_err(),
                "Expected '{}' to be invalid",
                fmt
            );
        }
    }

    #[test]
    fn test_uses_v2_features() {
        let v1_arg = ArgConfig {
            name: "test".to_string(),
            short: Some('t'),
            long: None,
            arg_type: ArgType::Flag,
            required: false,
            default: None,
            help: None,
            env: None,
            multiple: false,
            num_args: None,
            delimiter: None,
        };
        assert!(!v1_arg.uses_v2_features());

        let v2_arg_env = ArgConfig {
            env: Some("TEST".to_string()),
            ..v1_arg.clone()
        };
        assert!(v2_arg_env.uses_v2_features());

        let v2_arg_multiple = ArgConfig {
            multiple: true,
            ..v1_arg.clone()
        };
        assert!(v2_arg_multiple.uses_v2_features());

        let v2_arg_num_args = ArgConfig {
            num_args: Some("1..".to_string()),
            ..v1_arg.clone()
        };
        assert!(v2_arg_num_args.uses_v2_features());

        let v2_arg_delimiter = ArgConfig {
            delimiter: Some(','),
            ..v1_arg.clone()
        };
        assert!(v2_arg_delimiter.uses_v2_features());
    }

    #[test]
    fn test_effective_long_explicit() {
        // When long is explicitly specified, use it
        let arg = ArgConfig {
            name: "verbose".to_string(),
            short: Some('v'),
            long: Some("verbose".to_string()),
            arg_type: ArgType::Flag,
            required: false,
            default: None,
            help: None,
            env: None,
            multiple: false,
            num_args: None,
            delimiter: None,
        };
        assert_eq!(arg.effective_long(), Some("verbose"));
    }

    #[test]
    fn test_effective_long_fallback_to_name() {
        // When neither short nor long is specified, use name as long
        let arg = ArgConfig {
            name: "verbose".to_string(),
            short: None,
            long: None,
            arg_type: ArgType::Flag,
            required: false,
            default: None,
            help: None,
            env: None,
            multiple: false,
            num_args: None,
            delimiter: None,
        };
        assert_eq!(arg.effective_long(), Some("verbose"));
    }

    #[test]
    fn test_effective_long_with_short_only() {
        // When only short is specified, no long option
        let arg = ArgConfig {
            name: "verbose".to_string(),
            short: Some('v'),
            long: None,
            arg_type: ArgType::Flag,
            required: false,
            default: None,
            help: None,
            env: None,
            multiple: false,
            num_args: None,
            delimiter: None,
        };
        assert_eq!(arg.effective_long(), None);
    }

    #[test]
    fn test_effective_long_positional() {
        // Positional args never have long options
        let arg = ArgConfig {
            name: "input".to_string(),
            short: None,
            long: None,
            arg_type: ArgType::Positional,
            required: false,
            default: None,
            help: None,
            env: None,
            multiple: false,
            num_args: None,
            delimiter: None,
        };
        assert_eq!(arg.effective_long(), None);
    }
}
