//! shclap - Clap-style argument parsing for shell scripts.
//!
//! This library provides the core functionality for parsing command-line
//! arguments according to a JSON configuration, generating help text,
//! and outputting parsed values as shell export statements.

pub mod config;
pub mod help;
pub mod output;
pub mod parser;

pub use config::{ArgConfig, ArgType, Config, ConfigError, CURRENT_SCHEMA_VERSION};
pub use help::{generate_help, generate_version};
pub use output::{
    generate_error_output, generate_error_string, generate_help_output,
    generate_help_output_string, generate_output, generate_output_string, generate_version_output,
    generate_version_output_string,
};
pub use parser::{parse_args, ParseError, ParseOutcome, ParseResult};
