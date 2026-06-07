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
    let (_, stderr) = run_example("info");
    assert!(stderr.contains("error: "));
    assert!(stderr.contains("warn: "));
    assert!(!stderr.contains("debug: "));
    assert!(!stderr.contains("trace: "));
}

#[test]
fn trace_level_logs_all() {
    let (_, stderr) = run_example("trace");
    assert!(stderr.contains("error: "));
    assert!(stderr.contains("warn: "));
    assert!(stderr.contains("info: "));
    assert!(stderr.contains("debug: "));
    assert!(stderr.contains("trace: "));
}

#[test]
fn trace_format_includes_module_path() {
    let (_, stderr) = run_example("trace");
    for line in stderr.lines() {
        assert!(
            line.contains('(') && line.contains("): "),
            "expected module_path(line) prefix, got: {:?}",
            line
        );
    }
}

#[test]
fn stdout_is_separate_from_stderr() {
    let (stdout, _stderr) = run_example("cli");
    assert!(
        !stdout.is_empty(),
        "expected stdout content from cli example"
    );
    assert!(
        !stdout.contains("error:"),
        "stdout should not contain log output"
    );
}

#[test]
fn no_color_suppresses_ansi() {
    let (_, stderr) = run_example_with_env("info", "NO_COLOR", "1");
    assert!(
        !stderr.contains("\x1b["),
        "expected no ANSI codes with NO_COLOR"
    );
}

#[test]
fn force_color_enables_ansi() {
    let (_, stderr) = run_example_with_env("info", "FORCE_COLOR", "1");
    assert!(
        stderr.contains("\x1b["),
        "expected ANSI codes with FORCE_COLOR"
    );
}

#[test]
fn info_message_has_no_level_prefix() {
    let (_, stderr) = run_example("info");
    for line in stderr.lines() {
        if line.contains("without prefix or color") {
            assert!(
                !line.contains("info:"),
                "info line should not have prefix: {:?}",
                line
            );
        }
    }
}
