//! A simple, opinionated logger for command line tools
//!
//! `ocli` aims at a very simple thing: logging for CLI tools done right. It uses the
//! `log` crate and the `ansi_term` crate for colors. It provides very few configuration —
//! at this time, just the expected log level.
//!
//! ## Features
//!
//!  `ocli`:
//!
//! * logs everything to `stderr`. CLI tools are expected to be usable in a pipe. In that context,
//!   the messages addressed to the user must be written on `stderr` to have a chance to be read
//!   by the user, independently of the log level.
//!   The program outputs that are meant to be used with a pipe shouldn't go through the logging
//!   system, but instead be printed to `stdout`, for example with `println!`.
//! * shows the `Info` message as plain uncolored text. `Info` is expected to be the normal log
//!   level to display messages that are not highlighting a problem and that are not too verbose
//!   for a standard usage of the tool. Because it is intended for messages that are related
//!   to a normal situation, the messages of that level are not prefixed with the log level.
//! * prefix the messages with their colored log level for any level other than `Info`. The color
//!   depends on the log level, allowing to quickly locate a message at a specific log level
//! * displays the module path and line when configured at the `Trace` log level, for all the
//!   messages, even if they are not at the `Trace` log level. The `Trace` log level is used
//!   to help the developer understand where a message comes from, in addition to display a larger
//!   amount of messages.
//! * disables all colorization in case the `stderr` is not a tty, so the output is not polluted
//!   with unreadable characters when `stderr` is redirected to a file. This crates disables
//!   colorization when the `NO_COLOR` environment is set, and force it when `FORCE_COLOR` is set.
//!   The colorization is disabled when both environment variables are set.
//!
//! ## Example with `Info` log level
//!
//! ```rust
//! #[macro_use] extern crate log;
//!
//! fn main() {
//!      ocli::init(log::Level::Info).unwrap();
//!
//!      error!("This is printed to stderr, with the 'error: ' prefix colored in red");
//!      warn!("This is printed to stderr, with the 'warn: ' prefix colored in yellow");
//!      info!("This is printed to stderr, without prefix or color");
//!      debug!("This is not printed");
//!      trace!("This is not printed");
//! }
//! ```
//!
//! ## Example with `Trace` log level
//!
//! ```rust
//! #[macro_use] extern crate log;
//!
//! fn main() {
//!      ocli::init(log::Level::Trace).unwrap();
//!
//!      error!("This is printed to stderr, with the 'path(line): error: ' prefix colored in red");
//!      warn!("This is printed to stderr, with the 'path(line): warn: ' prefix colored in yellow");
//!      info!("This is printed to stderr, with the 'path(line): info: ' prefix");
//!      debug!("This is printed to stderr, with the 'path(line): debug: ' prefix colored in blue");
//!      trace!("This is printed to stderr, with the 'path(line): trace: ' prefix colored in magenta");
//! }
//! ```
//!
//! ## Example with log level configured with a command line option
//!
//! TODO: write a small example that uses clap derive
//!

use std::{
    env,
    io::{self, IsTerminal},
    sync::Arc,
};

use log::SetLoggerError;
use termcolor::{BufferWriter, Color, ColorChoice, ColorSpec, WriteColor};

pub const MODULE_PATH_UNKNOWN: &str = "?";
pub const MODULE_LINE_UNKNOWN: &str = "?";

#[derive(Debug)]
pub struct Logger {
    level: log::Level,
    writer: Arc<BufferWriter>,
}

impl Logger {
    /// Creates a new instance of the cli logger.
    ///
    /// The default level is Info.
    pub fn new() -> Logger {
        Logger {
            level: log::Level::Info,
            writer: Arc::new(BufferWriter::stderr(determine_color_choice())),
        }
    }

    /// Explicitly sets the log level.
    pub fn level(mut self, level: log::Level) -> Self {
        self.level = level;
        self
    }

    /// Initializes the logger.
    ///
    /// This also consumes the logger. It cannot be further modified after initialization.
    pub fn init(self) -> Result<(), SetLoggerError> {
        log::set_max_level(self.level.to_level_filter());
        log::set_boxed_logger(Box::new(self))
    }

    fn format_default(&self, record: &log::Record, w: &mut dyn WriteColor) -> io::Result<()> {
        let level = record.level();
        let msg = format!("{}", record.args());

        if matches!(level, log::Level::Info) {
            w.reset()?;
            write!(w, "{}", msg)?;
            return if msg.ends_with('\n') {
                Ok(())
            } else {
                writeln!(w)
            };
        }

        let prefix = format!("{}: ", level.to_string().to_lowercase());

        if msg.is_empty() {
            if let Some(color) = color(level) {
                w.set_color(ColorSpec::new().set_fg(Some(color)))?;
            }
            write!(w, "{}", prefix)?;
            w.reset()?;
            return writeln!(w);
        }

        for line in msg.lines() {
            if let Some(color) = color(level) {
                w.set_color(ColorSpec::new().set_fg(Some(color)))?;
            }
            write!(w, "{}{}\n", prefix, line)?;
            w.reset()?;
        }

        Ok(())
    }

    fn format_trace(&self, record: &log::Record, w: &mut dyn WriteColor) -> io::Result<()> {
        let path = record.module_path().unwrap_or(MODULE_PATH_UNKNOWN);
        let line = record
            .line()
            .map_or_else(|| MODULE_LINE_UNKNOWN.to_string(), |l| l.to_string());
        let level = record.level();
        let msg = format!("{}", record.args());
        let prefix = format!("{}({}): {}: ", path, line, level.to_string().to_lowercase());

        if msg.is_empty() {
            if let Some(color) = color(level) {
                w.set_color(ColorSpec::new().set_fg(Some(color)))?;
            }
            write!(w, "{}", prefix)?;
            w.reset()?;
            return writeln!(w);
        }

        for line in msg.lines() {
            if let Some(color) = color(level) {
                w.set_color(ColorSpec::new().set_fg(Some(color)))?;
            }
            write!(w, "{}{}\n", prefix, line)?;
            w.reset()?;
        }

        Ok(())
    }
}

impl log::Log for Logger {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        metadata.level() <= self.level
    }

    fn log(&self, record: &log::Record) {
        if self.enabled(record.metadata()) {
            let mut buffer = self.writer.buffer();
            let result = match self.level {
                log::Level::Trace => self.format_trace(record, &mut buffer),
                _ => self.format_default(record, &mut buffer),
            };
            result.expect("Failed to format log");
            self.writer.print(&buffer).expect("Failed to log");
        }
    }
    fn flush(&self) {
        // already done
    }
}

impl Default for Logger {
    fn default() -> Logger {
        Logger::new()
    }
}

/// Initializes the logger.
///
/// This also consumes the logger. It cannot be further modified after initialization.
///
/// # Example
///
/// ```rust
/// #[macro_use] extern crate log;
/// extern crate ocli;
///
/// fn main() {
///     ocli::init(log::Level::Info).unwrap();
///
///     error!("This is printed to stderr, with the 'error: ' prefix");
///     warn!("This is printed to stderr, with the 'warn: ' prefix");
///     info!("This is printed to stderr, without prefix");
///     debug!("This is not printed");
///     trace!("This is not printed");
/// }
/// ```
pub fn init(level: log::Level) -> Result<(), SetLoggerError> {
    Logger::new().level(level).init()
}

/// Determines whether colors should be used based on environment variables and terminal.
fn determine_color_choice() -> ColorChoice {
    let no_color = env::var("NO_COLOR");
    let force_color = env::var("FORCE_COLOR");
    if no_color.is_ok() && !no_color.unwrap().is_empty() {
        ColorChoice::Never
    } else if force_color.is_ok() && !force_color.unwrap().is_empty() {
        ColorChoice::Always
    } else if std::io::stderr().is_terminal() {
        ColorChoice::Auto
    } else {
        ColorChoice::Never
    }
}

/// Returns the color associated with the log level
fn color(level: log::Level) -> Option<Color> {
    match level {
        log::Level::Error => Some(Color::Red),
        log::Level::Warn => Some(Color::Yellow),
        log::Level::Info => None,
        log::Level::Debug => Some(Color::Blue),
        log::Level::Trace => Some(Color::Magenta),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use log::Log;

    // ── color() ──────────────────────────────────────────────────────

    #[test]
    fn color_error_is_red() {
        assert_eq!(color(log::Level::Error), Some(Color::Red));
    }

    #[test]
    fn color_warn_is_yellow() {
        assert_eq!(color(log::Level::Warn), Some(Color::Yellow));
    }

    #[test]
    fn color_info_is_none() {
        assert_eq!(color(log::Level::Info), None);
    }

    #[test]
    fn color_debug_is_blue() {
        assert_eq!(color(log::Level::Debug), Some(Color::Blue));
    }

    #[test]
    fn color_trace_is_magenta() {
        assert_eq!(color(log::Level::Trace), Some(Color::Magenta));
    }

    // ── Logger construction ──────────────────────────────────────────

    #[test]
    fn logger_default_level_is_info() {
        let logger = Logger::new();
        assert_eq!(logger.level, log::Level::Info);
    }

    #[test]
    fn logger_level_setter() {
        let logger = Logger::new().level(log::Level::Debug);
        assert_eq!(logger.level, log::Level::Debug);
    }

    #[test]
    fn logger_default_trait() {
        let logger = Logger::default();
        assert_eq!(logger.level, log::Level::Info);
    }

    // ── enabled() ────────────────────────────────────────────────────

    #[test]
    fn enabled_accepts_level_at_or_below_max() {
        let logger = Logger::new().level(log::Level::Warn);
        assert!(logger.enabled(&log::Metadata::builder().level(log::Level::Warn).build()));
        assert!(logger.enabled(&log::Metadata::builder().level(log::Level::Error).build()));
    }

    #[test]
    fn enabled_rejects_level_above_max() {
        let logger = Logger::new().level(log::Level::Warn);
        assert!(!logger.enabled(&log::Metadata::builder().level(log::Level::Info).build()));
        assert!(!logger.enabled(&log::Metadata::builder().level(log::Level::Debug).build()));
        assert!(!logger.enabled(&log::Metadata::builder().level(log::Level::Trace).build()));
    }

    // ── determine_color_choice() ────────────────────────────────────

    #[test]
    fn color_choice_no_color_disables() {
        temp_env::with_var("NO_COLOR", Some("1"), || {
            assert_eq!(determine_color_choice(), ColorChoice::Never);
        });
    }

    #[test]
    fn color_choice_force_color_enables() {
        temp_env::with_var("FORCE_COLOR", Some("1"), || {
            assert_eq!(determine_color_choice(), ColorChoice::Always);
        });
    }

    #[test]
    fn color_choice_no_color_wins_over_force_color() {
        temp_env::with_vars(
            vec![("NO_COLOR", Some("1")), ("FORCE_COLOR", Some("1"))],
            || {
                assert_eq!(determine_color_choice(), ColorChoice::Never);
            },
        );
    }

    #[test]
    fn color_choice_no_color_empty_does_not_disable() {
        temp_env::with_var("NO_COLOR", Some(""), || {
            let cc = determine_color_choice();
            assert_ne!(cc, ColorChoice::Always);
        });
    }

    #[test]
    fn color_choice_force_color_empty_does_not_enable() {
        temp_env::with_var("FORCE_COLOR", Some(""), || {
            let cc = determine_color_choice();
            assert_ne!(cc, ColorChoice::Always);
        });
    }

    // ── format_default() (non-Trace logger) ──────────────────────────

    fn format_default(
        level: log::Level,
        msg: &str,
        module_path: Option<&str>,
        line: Option<u32>,
    ) -> Vec<u8> {
        let logger = Logger::new();
        let mut buf = BufferWriter::stderr(ColorChoice::Never).buffer();
        let args = format_args!("{}", msg);
        let record = log::Record::builder()
            .args(args)
            .level(level)
            .target("test")
            .module_path(module_path)
            .line(line)
            .build();
        logger.format_default(&record, &mut buf).unwrap();
        buf.into_inner()
    }

    #[test]
    fn format_info_has_no_prefix() {
        let out = format_default(log::Level::Info, "hello", Some("m"), Some(1));
        assert_eq!(String::from_utf8(out).unwrap(), "hello\n");
    }

    #[test]
    fn format_error_has_error_prefix() {
        let out = format_default(log::Level::Error, "boom", Some("m"), Some(1));
        assert_eq!(String::from_utf8(out).unwrap(), "error: boom\n");
    }

    #[test]
    fn format_warn_has_warn_prefix() {
        let out = format_default(log::Level::Warn, "careful", Some("m"), Some(1));
        assert_eq!(String::from_utf8(out).unwrap(), "warn: careful\n");
    }

    #[test]
    fn format_debug_has_debug_prefix() {
        let out = format_default(log::Level::Debug, "debugging", Some("m"), Some(1));
        assert_eq!(String::from_utf8(out).unwrap(), "debug: debugging\n");
    }

    #[test]
    fn format_trace_at_default_has_trace_prefix() {
        let out = format_default(log::Level::Trace, "spam", Some("m"), Some(1));
        assert_eq!(String::from_utf8(out).unwrap(), "trace: spam\n");
    }

    #[test]
    fn format_error_multiline() {
        let out = format_default(log::Level::Error, "line1\nline2\nline3", Some("m"), Some(1));
        assert_eq!(
            String::from_utf8(out).unwrap(),
            "error: line1\nerror: line2\nerror: line3\n"
        );
    }

    #[test]
    fn format_info_multiline() {
        let out = format_default(log::Level::Info, "line1\nline2\nline3", Some("m"), Some(1));
        assert_eq!(String::from_utf8(out).unwrap(), "line1\nline2\nline3\n");
    }

    #[test]
    fn format_default_contains_ansi_for_non_info() {
        let logger = Logger::new();
        let mut buf = BufferWriter::stderr(ColorChoice::Always).buffer();
        let args = format_args!("{}", "boom");
        let record = log::Record::builder()
            .args(args)
            .level(log::Level::Error)
            .target("test")
            .module_path(Some("m"))
            .line(Some(1))
            .build();
        logger.format_default(&record, &mut buf).unwrap();
        let s = String::from_utf8(buf.into_inner()).unwrap();
        assert!(s.contains("\x1b["), "expected ANSI escape codes for Error");
    }

    #[test]
    fn format_default_no_ansi_for_info() {
        let out = format_default(log::Level::Info, "plain", Some("m"), Some(1));
        let s = String::from_utf8(out).unwrap();
        assert!(
            !s.contains("\x1b["),
            "expected no ANSI escape codes for Info"
        );
    }

    // ── format_trace() (Trace-level logger) ──────────────────────────

    fn format_trace(
        level: log::Level,
        msg: &str,
        module_path: Option<&str>,
        line: Option<u32>,
    ) -> Vec<u8> {
        let logger = Logger::new().level(log::Level::Trace);
        let mut buf = BufferWriter::stderr(ColorChoice::Never).buffer();
        let args = format_args!("{}", msg);
        let record = log::Record::builder()
            .args(args)
            .level(level)
            .target("test")
            .module_path(module_path)
            .line(line)
            .build();
        logger.format_trace(&record, &mut buf).unwrap();
        buf.into_inner()
    }

    #[test]
    fn trace_format_includes_module_and_line() {
        let out = format_trace(log::Level::Info, "hi", Some("my_mod"), Some(42));
        let s = String::from_utf8(out).unwrap();
        assert!(s.starts_with("my_mod(42): info: hi\n"), "got: {:?}", s);
    }

    #[test]
    fn trace_format_with_error_level() {
        let out = format_trace(log::Level::Error, "fail", Some("app"), Some(10));
        let s = String::from_utf8(out).unwrap();
        assert!(s.starts_with("app(10): error: fail\n"), "got: {:?}", s);
    }

    #[test]
    fn trace_format_missing_module_uses_question_mark() {
        let out = format_trace(log::Level::Warn, "warn", None, Some(5));
        let s = String::from_utf8(out).unwrap();
        assert!(s.starts_with("?(5): warn: warn\n"), "got: {:?}", s);
    }

    #[test]
    fn trace_format_multiline() {
        let out = format_trace(log::Level::Error, "line1\nline2", Some("mod"), Some(5));
        assert_eq!(
            String::from_utf8(out).unwrap(),
            "mod(5): error: line1\nmod(5): error: line2\n"
        );
    }

    #[test]
    fn trace_format_missing_line_uses_question_mark() {
        let out = format_trace(log::Level::Debug, "dbg", Some("mod"), None);
        let s = String::from_utf8(out).unwrap();
        assert!(s.starts_with("mod(?): debug: dbg\n"), "got: {:?}", s);
    }

    #[test]
    fn trace_format_contains_ansi() {
        let logger = Logger::new().level(log::Level::Trace);
        let mut buf = BufferWriter::stderr(ColorChoice::Always).buffer();
        let args = format_args!("{}", "err");
        let record = log::Record::builder()
            .args(args)
            .level(log::Level::Error)
            .target("test")
            .module_path(Some("m"))
            .line(Some(1))
            .build();
        logger.format_trace(&record, &mut buf).unwrap();
        let s = String::from_utf8(buf.into_inner()).unwrap();
        assert!(s.contains("\x1b["), "expected ANSI codes in trace format");
    }
}
