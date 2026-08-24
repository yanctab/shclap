//! Logging functionality for shclap.
//!
//! Provides leveled, colored logging output to stderr with environment variable
//! control over minimum log level.

use anyhow::{bail, Result};
use log::{debug, error, info, trace, warn};
use std::io::Write;
use std::sync::Once;

static LOGGER_INIT: Once = Once::new();

/// ANSI color codes for log levels
mod colors {
    pub const TRACE: &str = "\x1b[36m"; // Cyan
    pub const DEBUG: &str = "\x1b[35m"; // Magenta
    pub const INFO: &str = "\x1b[32m"; // Green
    pub const WARN: &str = "\x1b[33m"; // Yellow
    pub const ERROR: &str = "\x1b[31m"; // Red
    pub const RESET: &str = "\x1b[0m"; // Reset
}

/// Check if stderr is a TTY
fn is_stderr_tty() -> bool {
    atty::is(atty::Stream::Stderr)
}

/// Format a level string with optional color
fn format_level(level: &str, use_color: bool) -> String {
    let level_lower = level.to_lowercase();
    let level_display = if level_lower == "warn" {
        "WARNING"
    } else {
        &level.to_uppercase()
    };

    if use_color {
        let color = match level_lower.as_str() {
            "trace" => colors::TRACE,
            "debug" => colors::DEBUG,
            "info" => colors::INFO,
            "warn" => colors::WARN,
            "error" => colors::ERROR,
            _ => "",
        };
        format!("{}{}{}", color, level_display, colors::RESET)
    } else {
        level_display.to_string()
    }
}

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

    // Detect TTY for color output
    let use_color = is_stderr_tty();

    // Initialize env_logger with custom format (only once per process)
    LOGGER_INIT.call_once(|| {
        let mut builder = env_logger::Builder::new();

        // Set up custom format with optional colors
        let use_color_copy = use_color;
        builder.format(move |buf, record| {
            let level_str = record.level().as_str();
            let formatted_level = format_level(level_str, use_color_copy);
            writeln!(buf, "{}: {}", formatted_level, record.args())
        });

        // Set the filter level from SHCLAP_LOG, default to info
        let filter_level = std::env::var("SHCLAP_LOG").unwrap_or_else(|_| "info".to_string());
        builder.parse_filters(&filter_level);

        // Ensure output goes to stderr
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
