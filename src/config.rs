//! JSON configuration parsing and types for shclap.

use serde::Deserialize;
use thiserror::Error;

/// The currently supported schema version.
pub const CURRENT_SCHEMA_VERSION: u32 = 1;

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

    #[error("unsupported schema version {0} (supported: 1)")]
    UnsupportedSchemaVersion(u32),
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
    /// Name of the script
    pub name: String,
    /// Description of the script
    pub description: Option<String>,
    /// Version of the script
    pub version: Option<String>,
    /// Environment variable prefix (default: "SHCLAP_")
    pub prefix: Option<String>,
    /// List of argument configurations
    #[serde(default)]
    pub args: Vec<ArgConfig>,
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
        if self.schema_version != CURRENT_SCHEMA_VERSION {
            return Err(ConfigError::UnsupportedSchemaVersion(self.schema_version));
        }

        let mut names = HashSet::new();

        for arg in &self.args {
            // Check for duplicate names
            if !names.insert(&arg.name) {
                return Err(ConfigError::DuplicateName(arg.name.clone()));
            }

            // Validate short option
            if let Some(short) = arg.short {
                if !short.is_ascii_alphabetic() {
                    return Err(ConfigError::InvalidShortOption(short.to_string()));
                }
            }

            // For non-positional args, ensure at least short or long is specified
            if arg.arg_type != ArgType::Positional && arg.short.is_none() && arg.long.is_none() {
                return Err(ConfigError::NoOptionSpecified(arg.name.clone()));
            }
        }

        Ok(())
    }

    /// Get the effective prefix, using the default if none is set.
    pub fn effective_prefix(&self) -> &str {
        self.prefix.as_deref().unwrap_or("SHCLAP_")
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
        assert_eq!(config.name, "myscript");
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
        assert_eq!(config.name, "minimal");
        assert!(config.description.is_none());
        assert!(config.version.is_none());
        assert!(config.prefix.is_none());
        assert!(config.args.is_empty());
        config.validate().unwrap();
    }

    #[test]
    fn test_error_on_missing_name() {
        let json = r#"{"description": "no name"}"#;
        let result = Config::from_json(json);
        assert!(result.is_err());
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
    fn test_error_on_no_option_specified() {
        let json = r#"{
            "name": "test",
            "args": [
                {"name": "noopt", "type": "flag"}
            ]
        }"#;
        let config = Config::from_json(json).unwrap();
        let result = config.validate();
        assert!(matches!(result, Err(ConfigError::NoOptionSpecified(_))));
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
}
