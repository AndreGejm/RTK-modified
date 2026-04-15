//! Shared command execution skeleton for filter modules.

use anyhow::{Context, Result};
use std::process::Command;

use crate::core::tracking;
use crate::core::utils::{exit_code_from_output, exit_code_from_status};

fn normalize_trailing_newlines(text: &str) -> &str {
    text.trim_end_matches(&['\r', '\n'][..])
}

fn is_lossy_output(filtered: &str, source: &str) -> bool {
    normalize_trailing_newlines(filtered) != normalize_trailing_newlines(source)
}

fn format_filtered_output(filtered: &str, is_lossy: bool) -> String {
    if is_lossy {
        format!("[summary view]\n{}", filtered)
    } else {
        filtered.to_string()
    }
}

fn recovery_label<'a>(
    tool_name: &'a str,
    tee_label: Option<&'a str>,
    is_lossy: bool,
) -> Option<&'a str> {
    if is_lossy {
        Some(tee_label.unwrap_or(tool_name))
    } else {
        tee_label
    }
}

fn print_raw_output(stdout: &str, stderr: &str) {
    if !stdout.is_empty() {
        print!("{}", stdout);
        if !stdout.ends_with('\n') {
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

fn print_display_output(display: &str, hint: Option<String>, no_trailing_newline: bool) {
    if let Some(hint) = hint {
        println!("{}\n{}", display, hint);
    } else if no_trailing_newline {
        print!("{}", display);
    } else {
        println!("{}", display);
    }
}

#[derive(Debug, PartialEq)]
struct CustomOutputPlan<'a> {
    display: String,
    tracking_output: String,
    hint_label: Option<&'a str>,
    force_hint: bool,
    raw_mode: bool,
}

fn build_custom_output_plan<'a>(
    tool_name: &'a str,
    stdout: &str,
    stderr: &str,
    filtered: &str,
    source_for_lossiness: &str,
    tee_label: Option<&'a str>,
    exit_code: i32,
) -> CustomOutputPlan<'a> {
    let raw = format!("{}\n{}", stdout, stderr);

    if exit_code != 0 {
        return CustomOutputPlan {
            display: raw.clone(),
            tracking_output: raw,
            hint_label: Some(tee_label.unwrap_or(tool_name)),
            force_hint: true,
            raw_mode: true,
        };
    }

    let is_lossy = is_lossy_output(filtered, source_for_lossiness);
    let display = format_filtered_output(filtered, is_lossy);

    CustomOutputPlan {
        tracking_output: display.clone(),
        display,
        hint_label: recovery_label(tool_name, tee_label, is_lossy),
        force_hint: is_lossy,
        raw_mode: false,
    }
}

#[derive(Default)]
pub struct RunOptions<'a> {
    pub tee_label: Option<&'a str>,
    pub filter_stdout_only: bool,
    pub skip_filter_on_failure: bool,
    pub no_trailing_newline: bool,
}

impl<'a> RunOptions<'a> {
    pub fn with_tee(label: &'a str) -> Self {
        Self {
            tee_label: Some(label),
            ..Default::default()
        }
    }

    pub fn stdout_only() -> Self {
        Self {
            filter_stdout_only: true,
            ..Default::default()
        }
    }

    pub fn tee(mut self, label: &'a str) -> Self {
        self.tee_label = Some(label);
        self
    }

    pub fn early_exit_on_failure(mut self) -> Self {
        self.skip_filter_on_failure = true;
        self
    }

    pub fn no_trailing_newline(mut self) -> Self {
        self.no_trailing_newline = true;
        self
    }
}

pub fn run_filtered<F>(
    mut cmd: Command,
    tool_name: &str,
    args_display: &str,
    filter_fn: F,
    opts: RunOptions<'_>,
) -> Result<i32>
where
    F: Fn(&str) -> String,
{
    let timer = tracking::TimedExecution::start();

    let output = cmd
        .output()
        .with_context(|| format!("Failed to run {}", tool_name))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let raw = format!("{}\n{}", stdout, stderr);
    let exit_code = exit_code_from_output(&output, tool_name);

    if exit_code != 0 {
        print_raw_output(&stdout, &stderr);
        let failure_label = opts.tee_label.unwrap_or(tool_name);
        if let Some(hint) = crate::core::tee::force_tee_hint(&raw, failure_label)
            .or_else(|| crate::core::tee::tee_and_hint(&raw, failure_label, exit_code))
        {
            println!("{}", hint);
        }
        timer.track(
            &format!("{} {}", tool_name, args_display),
            &format!("rtk {} {}", tool_name, args_display),
            &raw,
            &raw,
        );
        return Ok(exit_code);
    }

    let text_to_filter = if opts.filter_stdout_only {
        &stdout
    } else {
        raw.as_str()
    };
    let filtered = filter_fn(text_to_filter);
    let is_lossy = is_lossy_output(&filtered, text_to_filter);
    let display = format_filtered_output(&filtered, is_lossy);
    let hint = recovery_label(tool_name, opts.tee_label, is_lossy).and_then(|label| {
        if is_lossy {
            crate::core::tee::force_tee_hint(&raw, label)
                .or_else(|| crate::core::tee::tee_and_hint(&raw, label, exit_code))
        } else {
            crate::core::tee::tee_and_hint(&raw, label, exit_code)
        }
    });

    print_display_output(&display, hint, opts.no_trailing_newline);

    if opts.filter_stdout_only && !stderr.trim().is_empty() {
        eprintln!("{}", stderr.trim());
    }

    let raw_for_tracking = if opts.filter_stdout_only {
        stdout.as_ref()
    } else {
        raw.as_str()
    };
    timer.track(
        &format!("{} {}", tool_name, args_display),
        &format!("rtk {} {}", tool_name, args_display),
        raw_for_tracking,
        &display,
    );

    Ok(exit_code)
}

#[allow(clippy::too_many_arguments)]
pub fn finish_custom_output(
    timer: &tracking::TimedExecution,
    command_label: &str,
    rtk_command_label: &str,
    tool_name: &str,
    stdout: &str,
    stderr: &str,
    filtered: &str,
    source_for_lossiness: &str,
    exit_code: i32,
    opts: RunOptions<'_>,
) -> i32 {
    let raw = format!("{}\n{}", stdout, stderr);
    let plan = build_custom_output_plan(
        tool_name,
        stdout,
        stderr,
        filtered,
        source_for_lossiness,
        opts.tee_label,
        exit_code,
    );

    let hint = plan.hint_label.and_then(|label| {
        if plan.force_hint {
            crate::core::tee::force_tee_hint(&raw, label)
                .or_else(|| crate::core::tee::tee_and_hint(&raw, label, exit_code))
        } else {
            crate::core::tee::tee_and_hint(&raw, label, exit_code)
        }
    });

    if plan.raw_mode {
        print_raw_output(stdout, stderr);
        if let Some(hint) = hint {
            println!("{}", hint);
        }
    } else {
        print_display_output(&plan.display, hint, opts.no_trailing_newline);
    }

    timer.track(
        command_label,
        rtk_command_label,
        &raw,
        &plan.tracking_output,
    );
    exit_code
}

pub fn run_passthrough(tool: &str, args: &[std::ffi::OsString], verbose: u8) -> Result<i32> {
    let timer = tracking::TimedExecution::start();
    if verbose > 0 {
        eprintln!("{} passthrough: {:?}", tool, args);
    }
    let status = crate::core::utils::resolved_command(tool)
        .args(args)
        .status()
        .with_context(|| format!("Failed to run {}", tool))?;
    let args_str = tracking::args_display(args);
    timer.track_passthrough(
        &format!("{} {}", tool, args_str),
        &format!("rtk {} {} (passthrough)", tool, args_str),
    );
    Ok(exit_code_from_status(&status, tool))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_lossy_output_ignores_trailing_newline_differences() {
        assert!(!is_lossy_output("ok\n", "ok"));
        assert!(!is_lossy_output("ok\r\n", "ok\n"));
    }

    #[test]
    fn test_format_filtered_output_marks_summary_views() {
        let formatted = format_filtered_output("Pytest: 5 passed", true);
        assert!(formatted.starts_with("[summary view]"));
        assert!(formatted.contains("Pytest: 5 passed"));
        assert_eq!(format_filtered_output("raw", false), "raw");
    }

    #[test]
    fn test_recovery_label_uses_tool_name_for_lossy_output_without_explicit_tee() {
        assert_eq!(recovery_label("mypy", None, true), Some("mypy"));
    }

    #[test]
    fn test_recovery_label_prefers_explicit_tee_label() {
        assert_eq!(
            recovery_label("pytest", Some("pytest_run"), true),
            Some("pytest_run")
        );
        assert_eq!(
            recovery_label("pytest", Some("pytest_run"), false),
            Some("pytest_run")
        );
    }

    #[test]
    fn test_build_custom_output_plan_uses_raw_output_on_failure() {
        let plan = build_custom_output_plan(
            "curl",
            "stdout body",
            "stderr body",
            "filtered",
            "stdout body",
            Some("curl_run"),
            22,
        );

        assert!(plan.raw_mode);
        assert_eq!(plan.display, "stdout body\nstderr body");
        assert_eq!(plan.tracking_output, "stdout body\nstderr body");
        assert_eq!(plan.hint_label, Some("curl_run"));
        assert!(plan.force_hint);
    }

    #[test]
    fn test_build_custom_output_plan_marks_lossy_success_as_summary() {
        let plan = build_custom_output_plan(
            "prisma",
            "Prisma Client generated\n  • 4 models",
            "",
            "Prisma Client generated",
            "Prisma Client generated\n  • 4 models",
            None,
            0,
        );

        assert!(!plan.raw_mode);
        assert!(plan.display.starts_with("[summary view]"));
        assert_eq!(plan.tracking_output, plan.display);
        assert_eq!(plan.hint_label, Some("prisma"));
        assert!(plan.force_hint);
    }

    #[test]
    fn test_build_custom_output_plan_keeps_exact_success_without_summary_label() {
        let plan = build_custom_output_plan(
            "gt",
            "ok sync: 2 synced",
            "",
            "ok sync: 2 synced",
            "ok sync: 2 synced",
            Some("gt_sync"),
            0,
        );

        assert!(!plan.raw_mode);
        assert_eq!(plan.display, "ok sync: 2 synced");
        assert_eq!(plan.tracking_output, "ok sync: 2 synced");
        assert_eq!(plan.hint_label, Some("gt_sync"));
        assert!(!plan.force_hint);
    }
}
