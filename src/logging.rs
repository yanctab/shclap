//! Logging functionality for shclap.
//!
//! Provides leveled, colored logging output to stderr with environment variable
//! control over minimum log level.

use anyhow::{bail, Result};
use log::{debug, error, info, trace, warn};
use std::io::{IsTerminal, Write};
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

/// Check if stderr is a TTY.
///
/// Uses `std::io::IsTerminal` (stable since Rust 1.70) rather than the `atty`
/// crate, which is unmaintained and carries RUSTSEC-2021-0145.
fn is_stderr_tty() -> bool {
    std::io::stderr().is_terminal()
}

/// Decide whether log output should carry ANSI color codes.
///
/// `SHCLAP_LOG_STYLE` selects the policy, matching the documented values:
/// - `always` — always color
/// - `never` — never color
/// - `auto` (the default, and the fallback for any unrecognized value) — color
///   only when stderr is a terminal
///
/// Under `auto` a non-empty `NO_COLOR` also suppresses color, following the
/// no-color.org convention. An explicit `always` is a deliberate override and
/// wins over `NO_COLOR`.
///
/// Taking the environment as parameters keeps this decision testable without
/// mutating process state, which would race the other tests in this binary.
fn should_use_color(style: Option<&str>, no_color: Option<&str>, stderr_is_tty: bool) -> bool {
    match style.map(|s| s.trim().to_lowercase()).as_deref() {
        Some("always") => true,
        Some("never") => false,
        // auto, unset, or anything unrecognized
        _ => stderr_is_tty && no_color.is_none_or(|v| v.is_empty()),
    }
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

    // Initialize env_logger with custom format (only once per process)
    init_once();

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

/// Initialize the logger once, allowing other modules to use log macros.
///
/// This function must be called before using log macros in other modules.
/// It is safe to call multiple times; the logger is only initialized once.
pub fn init_once() {
    LOGGER_INIT.call_once(|| {
        let mut builder = env_logger::Builder::new();

        // Resolve the color policy from SHCLAP_LOG_STYLE, falling back to TTY
        // detection. This was previously hard-wired to TTY detection alone, so
        // SHCLAP_LOG_STYLE was documented and forwarded into containers but had
        // no effect on output.
        let use_color_copy = should_use_color(
            std::env::var("SHCLAP_LOG_STYLE").ok().as_deref(),
            std::env::var("NO_COLOR").ok().as_deref(),
            is_stderr_tty(),
        );

        // env_logger writes through anstream, which strips ANSI sequences when
        // its own write style resolves to "no color" — which, under the default
        // Auto, it does whenever stderr is not a terminal. Without this the
        // codes emitted below would be silently removed and SHCLAP_LOG_STYLE
        // would still have no visible effect when piped.
        builder.write_style(if use_color_copy {
            env_logger::WriteStyle::Always
        } else {
            env_logger::WriteStyle::Never
        });

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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_style_always_forces_color() {
        assert!(should_use_color(Some("always"), None, false));
        // An explicit request wins over NO_COLOR.
        assert!(should_use_color(Some("always"), Some("1"), false));
    }

    #[test]
    fn test_style_never_suppresses_color() {
        assert!(!should_use_color(Some("never"), None, true));
    }

    #[test]
    fn test_style_auto_follows_tty() {
        assert!(should_use_color(Some("auto"), None, true));
        assert!(!should_use_color(Some("auto"), None, false));
    }

    #[test]
    fn test_unset_style_defaults_to_auto() {
        assert!(should_use_color(None, None, true));
        assert!(!should_use_color(None, None, false));
    }

    #[test]
    fn test_unrecognized_style_falls_back_to_auto() {
        assert!(should_use_color(Some("purple"), None, true));
        assert!(!should_use_color(Some("purple"), None, false));
    }

    #[test]
    fn test_style_is_case_and_space_insensitive() {
        assert!(should_use_color(Some("ALWAYS"), None, false));
        assert!(!should_use_color(Some("  Never  "), None, true));
    }

    #[test]
    fn test_no_color_suppresses_under_auto() {
        assert!(!should_use_color(None, Some("1"), true));
        // Per no-color.org an empty value does not count as set.
        assert!(should_use_color(None, Some(""), true));
        // Any non-empty value counts, including "0".
        assert!(!should_use_color(None, Some("0"), true));
    }

    #[test]
    fn test_format_level_colors_only_when_enabled() {
        assert_eq!(format_level("info", false), "INFO");
        assert!(format_level("info", true).contains(colors::INFO));
        assert!(format_level("info", true).contains(colors::RESET));
    }

    #[test]
    fn test_format_level_renders_warn_as_warning() {
        assert_eq!(format_level("warn", false), "WARNING");
    }

    #[test]
    fn test_run_rejects_unknown_level() {
        let err = run("shout", &["hi".to_string()]).unwrap_err();
        assert!(err.to_string().contains("unrecognized log level"));
    }
}
