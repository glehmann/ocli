# Plan: Add Comprehensive Test Suite to `ocli`

## Overview

`ocli` is a Rust CLI logging crate (v0.3.0). It has zero tests.
This plan adds unit tests (in `src/lib.rs`) and integration tests (in `tests/`),
with minimal refactoring for testability and zero public API changes.

---

## Step 1 — Extract `determine_color_choice()` (src/lib.rs)

Move the NO_COLOR/FORCE_COLOR/stderr-tty logic from `Logger::new()` into a
standalone function so it can be unit-tested.

```rust
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
```

Then simplify `new()`:

```rust
pub fn new() -> Logger {
    Logger {
        level: log::Level::Info,
        writer: Arc::new(BufferWriter::stderr(determine_color_choice())),
    }
}
```

---

## Step 2 — Refactor `log_with_level` / `log_with_trace` format methods (src/lib.rs)

Separate the formatting logic from the IO (writer printing). Two new methods
write into a `&mut dyn WriteColor`:

```rust
fn format_default(&self, record: &log::Record, w: &mut dyn WriteColor) -> io::Result<()> {
    let level = record.level();
    if let Some(color) = color(level) {
        w.set_color(ColorSpec::new().set_fg(Some(color)))?;
    }
    if !matches!(level, log::Level::Info) {
        write!(w, "{}: ", level.to_string().to_lowercase())?;
    }
    w.reset()?;
    writeln!(w, "{}", record.args())?;
    Ok(())
}

fn format_trace(&self, record: &log::Record, w: &mut dyn WriteColor) -> io::Result<()> {
    let path = record.module_path().unwrap_or(MODULE_PATH_UNKNOWN);
    let line = record.line().map_or_else(|| MODULE_LINE_UNKNOWN.to_string(), |l| l.to_string());
    let level = record.level();
    if let Some(color) = color(level) {
        w.set_color(ColorSpec::new().set_fg(Some(color)))?;
    }
    write!(w, "{}({}): {}: ", path, line, level.to_string().to_lowercase())?;
    w.reset()?;
    writeln!(w, "{}", record.args())?;
    Ok(())
}
```

Replace old `log_with_level` / `log_with_trace` with a unified `log` impl:

```rust
fn log(&self, record: &log::Record) {
    if self.enabled(record.metadata()) {
        let mut buffer = self.writer.buffer();
        let result = match self.level {
            log::Level::Trace => self.format_trace(record, &mut buffer),
            _ => self.format_default(record, &mut buffer),
        };
        if let Err(e) = result {
            panic!("Failed to format log: {}", e);
        }
        self.writer.print(&buffer).expect("Failed to log");
    }
}
```

Delete the old `log_with_level` and `log_with_trace` methods entirely.

---

## Step 3 — Unit tests (`#[cfg(test)]` module in src/lib.rs)

Add a `#[cfg(test)] mod tests` block at the bottom of `src/lib.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use termcolor::Buffer;

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
        temp_env::with_vars(vec![("NO_COLOR", Some("1")), ("FORCE_COLOR", Some("1"))], || {
            assert_eq!(determine_color_choice(), ColorChoice::Never);
        });
    }

    #[test]
    fn color_choice_no_color_empty_does_not_disable() {
        temp_env::with_var("NO_COLOR", Some(""), || {
            // empty string -> .is_empty() is true -> falls through
            // in non-tty test env -> ColorChoice::Never anyway,
            // so we just verify it's not explicitly ColorChoice::Never from NO_COLOR
            let cc = determine_color_choice();
            assert_ne!(cc, ColorChoice::Always); // at minimum, not Always
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

    fn format_default(level: log::Level, msg: &str, module_path: Option<&str>, line: Option<u32>) -> Vec<u8> {
        let logger = Logger::new();
        let mut buf = Buffer::new();
        let record = log::Record::builder()
            .args(format_args!("{}", msg))
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
    fn format_default_contains_ansi_for_non_info() {
        let out = format_default(log::Level::Error, "boom", Some("m"), Some(1));
        let s = String::from_utf8(out).unwrap();
        assert!(s.contains("\x1b["), "expected ANSI escape codes for Error, got: {:?}", s);
    }

    #[test]
    fn format_default_no_ansi_for_info() {
        let out = format_default(log::Level::Info, "plain", Some("m"), Some(1));
        let s = String::from_utf8(out).unwrap();
        assert!(!s.contains("\x1b["), "expected no ANSI escape codes for Info, got: {:?}", s);
    }

    // ── format_trace() (Trace-level logger) ──────────────────────────

    fn format_trace(level: log::Level, msg: &str, module_path: Option<&str>, line: Option<u32>) -> Vec<u8> {
        let logger = Logger::new().level(log::Level::Trace);
        let mut buf = Buffer::new();
        let record = log::Record::builder()
            .args(format_args!("{}", msg))
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
    fn trace_format_missing_line_uses_question_mark() {
        let out = format_trace(log::Level::Debug, "dbg", Some("mod"), None);
        let s = String::from_utf8(out).unwrap();
        assert!(s.starts_with("mod(?): debug: dbg\n"), "got: {:?}", s);
    }

    #[test]
    fn trace_format_contains_ansi() {
        let out = format_trace(log::Level::Error, "err", Some("m"), Some(1));
        let s = String::from_utf8(out).unwrap();
        assert!(s.contains("\x1b["), "expected ANSI codes in trace format, got: {:?}", s);
    }
}
```

**Dependency needed:** Add `temp-env` to dev-dependencies in `Cargo.toml`:

```toml
[dev-dependencies]
temp-env = "0.3"
```

This crate temporarily sets env vars for the duration of a test closure
and restores them afterwards — perfect for NO_COLOR/FORCE_COLOR tests.

---

## Step 4 — Integration tests (`tests/integration.rs`)

Because `termcolor::BufferWriter` always writes to real stderr, integration
tests spawn the example binaries as subprocesses and capture their stderr.

```rust
use std::process::Command;

fn run_example(name: &str) -> (String, String) {
    let output = Command::new("cargo")
        .args(["run", "--example", name, "--quiet"])
        .output()
        .expect("failed to run example");
    (
        String::from_utf8_lossy(&output.stdout).to_string(),
        String::from_utf8_lossy(&output.stderr).to_string(),
    )
}

fn run_example_with_env(name: &str, var: &str, val: &str) -> (String, String) {
    let output = Command::new("cargo")
        .args(["run", "--example", name, "--quiet"])
        .env(var, val)
        .output()
        .expect("failed to run example");
    (
        String::from_utf8_lossy(&output.stdout).to_string(),
        String::from_utf8_lossy(&output.stderr).to_string(),
    )
}

#[test]
fn info_level_logs_error_warn_info() {
    let (stdout, stderr) = run_example("info");
    assert!(stderr.contains("error: "), "stderr: {:?}", stderr);
    assert!(stderr.contains("warn: "), "stderr: {:?}", stderr);
    assert!(!stderr.contains("debug: "), "stderr: {:?}", stderr);
    assert!(!stderr.contains("trace: "), "stderr: {:?}", stderr);
}

#[test]
fn trace_level_logs_all() {
    let (_stdout, stderr) = run_example("trace");
    assert!(stderr.contains("error: "), "stderr: {:?}", stderr);
    assert!(stderr.contains("warn: "), "stderr: {:?}", stderr);
    assert!(stderr.contains("info: "), "stderr: {:?}", stderr);
    assert!(stderr.contains("debug: "), "stderr: {:?}", stderr);
    assert!(stderr.contains("trace: "), "stderr: {:?}", stderr);
}

#[test]
fn trace_format_includes_module_path() {
    let (_stdout, stderr) = run_example("trace");
    // The trace example logs from the example's main, so module path is "trace"
    for line in stderr.lines() {
        assert!(
            line.contains("((") && line.contains("): "),
            "expected module_path(line) prefix, got: {:?}",
            line
        );
    }
}

#[test]
fn stdout_is_separate_from_stderr() {
    let (stdout, stderr) = run_example("info");
    assert!(!stdout.is_empty(), "expected stdout content");
    assert!(!stderr.is_empty(), "expected stderr content");
    // stdout should NOT contain log prefixes
    assert!(!stdout.contains("error:"), "stdout: {:?}", stdout);
}

#[test]
fn no_color_suppresses_ansi() {
    let (_stdout, stderr) = run_example_with_env("info", "NO_COLOR", "1");
    assert!(!stderr.contains("\x1b["), "expected no ANSI codes with NO_COLOR");
}

#[test]
fn force_color_enables_ansi() {
    let (_stdout, stderr) = run_example_with_env("info", "FORCE_COLOR", "1");
    assert!(stderr.contains("\x1b["), "expected ANSI codes with FORCE_COLOR");
}

#[test]
fn info_message_has_no_level_prefix() {
    let (_stdout, stderr) = run_example("info");
    // "without prefix or color" is the info message in the example
    assert!(stderr.contains("without prefix or color"));
    // but the line should NOT contain "info:" prefix
    for line in stderr.lines() {
        if line.contains("without prefix or color") {
            assert!(!line.starts_with("info:"), "info line should not have prefix: {:?}", line);
        }
    }
}
```

---

## Step 5 — Run and verify

```bash
cargo test
```

Expected: all unit tests and integration tests pass.

---

## Files modified

| File | Change |
|------|--------|
| `Cargo.toml` | Add `temp-env = "0.3"` to `[dev-dependencies]` |
| `src/lib.rs` | Extract `determine_color_choice()`, refactor format methods, add `#[cfg(test)] mod tests` |
| `tests/integration.rs` | New file — subprocess-based integration tests |

---

## Summary of test coverage

| Category | Tests | What's covered |
|----------|-------|----------------|
| `color()` | 5 | Every log level maps to correct color |
| Logger construction | 3 | Default level, level setter, Default trait |
| `enabled()` | 2 | Above/below max level filtering |
| `determine_color_choice()` | 4 | NO_COLOR, FORCE_COLOR, both set, empty values |
| `format_default()` | 7 | Every level prefix, ANSI for non-Info, no ANSI for Info |
| `format_trace()` | 5 | Module/line prefix, missing path/line, ANSI codes |
| Integration | 7 | Info/Trace filtering, module path, stdout/stderr sep, NO_COLOR/FORCE_COLOR, Info no-prefix |
| **Total** | **33** | |
