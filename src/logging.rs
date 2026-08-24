//! Logging functionality for shclap.
//!
//! Provides leveled, colored logging output to stderr with environment variable
//! control over minimum log level.

use anyhow::{bail, Result};
use log::{debug, error, info, trace, warn};
use std::io::Write;
use std::sync::Once;

static LOGGER_INIT: Once = Once::new();

/// Run the logging subcommand.
///
/// Initializes env_logger with a custom format, sets the filter level from
/// SHCLAP_LOG environment variable, and dispatches to the appropriate log macro.
pub fn run(level: &str, message: &[String]) -> Result<()> {
    // Validate the level
    let level_lower = level.to_lowercase();
    if !["trace", "debug", "info", "warn", "error"].contains(&level_lower.as_str()) {
        bail!("unrecognized log level: {}", level);
    }

    // Initialize env_logger with custom format (only once per process)
    LOGGER_INIT.call_once(|| {
        let mut builder = env_logger::Builder::new();

        // Set up custom format: LEVEL: <message>
        builder.format(|buf, record| {
            writeln!(buf, "{}: {}", record.level().to_string().to_uppercase(), record.args())
        });

        // Set the filter level from SHCLAP_LOG, default to info
        let filter_level = std::env::var("SHCLAP_LOG").unwrap_or_else(|_| "info".to_string());
        builder.parse_filters(&filter_level);

        // Ensure output goes to stderr and detect TTY
        builder.target(env_logger::Target::Stderr);

        // Initialize the logger
        let _ = builder.try_init();
    });

    // Join the message parts
    let message_text = message.join(" ");

    // Dispatch to the appropriate log macro
    match level_lower.as_str() {
        "trace" => trace!("{}", message_text),
        "debug" => debug!("{}", message_text),
        "info" => info!("{}", message_text),
        "warn" => warn!("{}", message_text),
        "error" => error!("{}", message_text),
        _ => bail!("unrecognized log level: {}", level),
    }

    Ok(())
}
