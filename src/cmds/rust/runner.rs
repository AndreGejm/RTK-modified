//! Runs arbitrary commands with raw failures and compact success summaries.

use crate::core::policy::{classify_command, EvidenceSensitivity};
use crate::core::tracking;
use anyhow::{Context, Result};
use regex::Regex;
use std::process::{Command, Stdio};

/// Run a command and summarize only successful error/warning scans.
pub fn run_err(command: &str, verbose: u8) -> Result<i32> {
    let timer = tracking::TimedExecution::start();
    debug_assert_eq!(classify_command("err"), EvidenceSensitivity::EvidenceSensitive);

    if verbose > 0 {
        eprintln!("Running: {}", command);
    }

    let output = shell_output(command).context("Failed to execute command")?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let raw = join_output_streams(&stdout, &stderr);
    let exit_code = crate::core::utils::exit_code_from_output(&output, "err");

    if exit_code != 0 {
        print_raw_output(&stdout, &stderr);
        if let Some(hint) = crate::core::tee::force_tee_hint(&raw, "err")
            .or_else(|| crate::core::tee::tee_and_hint(&raw, "err", exit_code))
        {
            println!("{}", hint);
        }
        timer.track(command, "rtk run-err", &raw, &raw);
        return Ok(exit_code);
    }

    let filtered = filter_errors(&raw);
    let summary = if filtered.is_empty() {
        "[ok] Command completed successfully (no errors)".to_string()
    } else {
        filtered
    };
    let display = format!("[summary view]\n{}", summary);

    if let Some(hint) = crate::core::tee::force_tee_hint(&raw, "err") {
        println!("{}\n{}", display, hint);
    } else {
        println!("{}", display);
    }
    timer.track(command, "rtk run-err", &raw, &display);
    Ok(exit_code)
}

/// Run tests with raw failures and summary-only success output.
pub fn run_test(command: &str, verbose: u8) -> Result<i32> {
    let timer = tracking::TimedExecution::start();
    debug_assert_eq!(classify_command("test"), EvidenceSensitivity::EvidenceSensitive);

    if verbose > 0 {
        eprintln!("Running tests: {}", command);
    }

    let output = shell_output(command).context("Failed to execute test command")?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let raw = join_output_streams(&stdout, &stderr);
    let exit_code = crate::core::utils::exit_code_from_output(&output, "test");

    if exit_code != 0 {
        print_raw_output(&stdout, &stderr);
        if let Some(hint) = crate::core::tee::force_tee_hint(&raw, "test")
            .or_else(|| crate::core::tee::tee_and_hint(&raw, "test", exit_code))
        {
            println!("{}", hint);
        }
        timer.track(command, "rtk run-test", &raw, &raw);
        return Ok(exit_code);
    }

    let summary = extract_test_summary(&raw, command);
    let display = format!("[summary view]\n{}", summary);
    if let Some(hint) = crate::core::tee::force_tee_hint(&raw, "test") {
        println!("{}\n{}", display, hint);
    } else {
        println!("{}", display);
    }
    timer.track(command, "rtk run-test", &raw, &display);
    Ok(exit_code)
}

fn shell_output(command: &str) -> std::io::Result<std::process::Output> {
    if cfg!(target_os = "windows") {
        Command::new("cmd")
            .args(["/C", command])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
    } else {
        Command::new("sh")
            .args(["-c", command])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
    }
}

fn join_output_streams(stdout: &str, stderr: &str) -> String {
    match (stdout.is_empty(), stderr.is_empty()) {
        (true, true) => String::new(),
        (false, true) => stdout.to_string(),
        (true, false) => stderr.to_string(),
        (false, false) => {
            if stdout.ends_with('\n') {
                format!("{}{}", stdout, stderr)
            } else {
                format!("{}\n{}", stdout, stderr)
            }
        }
    }
}

fn print_raw_output(stdout: &str, stderr: &str) {
    if !stdout.is_empty() {
        print!("{}", stdout);
        if !stdout.ends_with('\n') && !stderr.is_empty() {
            println!();
        }
    }
    if !stderr.is_empty() {
        eprint!("{}", stderr);
        if !stderr.ends_with('\n') {
            eprintln!();
        }
    }
}

fn filter_errors(output: &str) -> String {
    lazy_static::lazy_static! {
        static ref ERROR_PATTERNS: Vec<Regex> = vec![
            Regex::new(r"(?i)^.*error[\s:\[].*$").unwrap(),
            Regex::new(r"(?i)^.*\berr\b.*$").unwrap(),
            Regex::new(r"(?i)^.*warning[\s:\[].*$").unwrap(),
            Regex::new(r"(?i)^.*\bwarn\b.*$").unwrap(),
            Regex::new(r"(?i)^.*failed.*$").unwrap(),
            Regex::new(r"(?i)^.*failure.*$").unwrap(),
            Regex::new(r"(?i)^.*exception.*$").unwrap(),
            Regex::new(r"(?i)^.*panic.*$").unwrap(),
            Regex::new(r"^error\[E\d+\]:.*$").unwrap(),
            Regex::new(r"^\s*--> .*:\d+:\d+$").unwrap(),
            Regex::new(r"^Traceback.*$").unwrap(),
            Regex::new(r#"^\s*File ".*", line \d+.*$"#).unwrap(),
            Regex::new(r"^\s*at .*:\d+:\d+.*$").unwrap(),
            Regex::new(r"^.*\.go:\d+:.*$").unwrap(),
        ];
    }

    let mut result = Vec::new();
    let mut in_error_block = false;
    let mut blank_count = 0;

    for line in output.lines() {
        let is_error_line = ERROR_PATTERNS.iter().any(|p| p.is_match(line));

        if is_error_line {
            in_error_block = true;
            blank_count = 0;
            result.push(line.to_string());
        } else if in_error_block {
            if line.trim().is_empty() {
                blank_count += 1;
                if blank_count >= 2 {
                    in_error_block = false;
                } else {
                    result.push(line.to_string());
                }
            } else if line.starts_with(' ') || line.starts_with('\t') {
                result.push(line.to_string());
                blank_count = 0;
            } else {
                in_error_block = false;
            }
        }
    }

    result.join("\n")
}

fn extract_test_summary(output: &str, command: &str) -> String {
    let mut result = Vec::new();
    let lines: Vec<&str> = output.lines().collect();

    let is_cargo = command.contains("cargo test");
    let is_pytest = command.contains("pytest");
    let is_jest =
        command.contains("jest") || command.contains("npm test") || command.contains("yarn test");
    let is_go = command.contains("go test");

    let mut failures = Vec::new();
    let mut in_failure = false;
    let mut failure_lines = Vec::new();

    for line in lines.iter() {
        if is_cargo {
            if line.contains("test result:") {
                result.push(line.to_string());
            }
            if line.contains("FAILED") && !line.contains("test result") {
                failures.push(line.to_string());
            }
            if line.starts_with("failures:") {
                in_failure = true;
            }
            if in_failure && line.starts_with("    ") {
                failure_lines.push(line.to_string());
            }
        }

        if is_pytest {
            if line.contains(" passed") || line.contains(" failed") || line.contains(" error") {
                result.push(line.to_string());
            }
            if line.contains("FAILED") {
                failures.push(line.to_string());
            }
        }

        if is_jest {
            if line.contains("Tests:") || line.contains("Test Suites:") {
                result.push(line.to_string());
            }
            if line.contains("✕") || line.contains("FAIL") {
                failures.push(line.to_string());
            }
        }

        if is_go {
            if line.starts_with("ok") || line.starts_with("FAIL") || line.starts_with("---") {
                result.push(line.to_string());
            }
            if line.contains("FAIL") {
                failures.push(line.to_string());
            }
        }
    }

    let mut output = String::new();

    if !failures.is_empty() {
        output.push_str("[FAIL] FAILURES:\n");
        for f in failures.iter().take(10) {
            output.push_str(&format!("  {}\n", f));
        }
        if failures.len() > 10 {
            output.push_str(&format!("  ... +{} more failures\n", failures.len() - 10));
        }
        output.push('\n');
    }

    if !result.is_empty() {
        output.push_str("SUMMARY:\n");
        for r in &result {
            output.push_str(&format!("  {}\n", r));
        }
    } else {
        output.push_str("OUTPUT (last 5 lines):\n");
        let start = lines.len().saturating_sub(5);
        for line in &lines[start..] {
            if !line.trim().is_empty() {
                output.push_str(&format!("  {}\n", line));
            }
        }
    }

    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_filter_errors() {
        let output = "info: compiling\nerror: something failed\n  at line 10\ninfo: done";
        let filtered = filter_errors(output);
        assert!(filtered.contains("error"));
        assert!(!filtered.contains("info"));
    }

    #[test]
    fn test_err_policy_is_evidence_sensitive() {
        assert_eq!(classify_command("err"), EvidenceSensitivity::EvidenceSensitive);
    }
}
