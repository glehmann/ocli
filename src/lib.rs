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
//! * disable all colorization in case the `stderr` is not a tty, so the output is not polluted
//!   with unreadable characters when `stderr` is redirected to a file.
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
//!      info!(This is printed to stderr, with the 'path(line): info: ' prefix");
//!      debug!("This is printed to stderr, with the 'path(line): debug: ' prefix colored in blue");
//!      trace!(This is printed to stderr, with the 'path(line): trace: ' prefix colored in magenta");
//! }
//! ```
//!
//! ## Example with log level configured with a command line option
//!
//! TODO: write a small example that uses clap derive
//!

use std::{
    io::{self, IsTerminal, Write},
    sync::Arc,
};

use log::SetLoggerError;
use termcolor::{BufferWriter, Color, ColorChoice, ColorSpec, WriteColor};

pub const MODULE_PATH_UNKNOWN: &str = "?";
pub const MODULE_LINE_UNKNOWN: &str = "?";

#[derive(Debug, Clone)]
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
            writer: Arc::new(BufferWriter::stderr(ColorChoice::Auto)),
        }
    }

    /// Explicitly sets the log level.
    pub fn level(mut self, l: log::Level) -> Self {
        self.level = l;
        self
    }

    /// Initializes the logger.
    ///
    /// This also consumes the logger. It cannot be further modified after initialization.
    pub fn init(self) -> Result<(), SetLoggerError> {
        log::set_max_level(self.level.to_level_filter());
        log::set_boxed_logger(Box::new(self))
    }

    fn log_with_level(&self, record: &log::Record) -> io::Result<()> {
        let level = record.level();

        let mut buffer = self.writer.buffer();

        // Set the header color
        buffer.set_color(ColorSpec::new().set_fg(Some(color(level))))?;

        if !matches!(level, log::Level::Info) {
            write!(buffer, "{}: ", level.to_string().to_lowercase())?;
        }

        // Reset the color to default
        buffer.reset()?;
        writeln!(buffer, "{}", record.args())?;

        self.writer.print(&buffer)
    }

    fn log_with_trace(&self, record: &log::Record) -> io::Result<()> {
        let path = record.module_path().unwrap_or(MODULE_PATH_UNKNOWN);
        let line = if let Some(l) = record.line() {
            l.to_string()
        } else {
            MODULE_LINE_UNKNOWN.to_string()
        };

        let level = record.level();

        let mut buffer = self.writer.buffer();

        // Set the header color
        buffer.set_color(ColorSpec::new().set_fg(Some(color(level))))?;

        write!(
            buffer,
            "{}({}): {}: ",
            path,
            line,
            level.to_string().to_lowercase()
        )?;

        // Reset the color to default
        buffer.reset()?;

        writeln!(buffer, "{}", record.args())?;

        self.writer.print(&buffer)
    }
}

impl log::Log for Logger {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        metadata.level() <= self.level
    }

    fn log(&self, record: &log::Record) {
        if self.enabled(record.metadata()) {
            match self.level {
                log::Level::Trace => self.log_with_trace(record).unwrap(),
                _ => self.log_with_level(record).unwrap(),
            }
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
///     warn!("This is printed to stderr, with the 'warn: ' prefix"");
///     info!("This is printed to stderr, without prefix");
///     debug!("This is not printed");
///     trace!("This is not printed");
/// }
/// ```
pub fn init(level: log::Level) -> Result<(), SetLoggerError> {
    Logger::new().level(level).init()
}

/// Returns the color associated with the log level
fn color(level: log::Level) -> Color {
    if std::io::stderr().is_terminal() {
        match level {
            log::Level::Error => Color::Red,
            log::Level::Warn => Color::Yellow,
            log::Level::Info => Color::White,
            log::Level::Debug => Color::Blue,
            log::Level::Trace => Color::Magenta,
        }
    } else {
        Color::White
    }
}
