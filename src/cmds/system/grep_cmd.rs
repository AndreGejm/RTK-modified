//! Search results are raw by default; compact grouping is opt-in.

use crate::core::config;
use crate::core::policy::{classify_command, EvidenceSensitivity};
use crate::core::tracking;
use crate::core::utils::{exit_code_from_output, resolved_command};
use anyhow::{Context, Result};
use regex::Regex;
use std::collections::HashMap;
use std::process::Stdio;

#[allow(clippy::too_many_arguments)]
pub fn run(
    pattern: &str,
    path: &str,
    max_line_len: usize,
    max_results: usize,
    context_only: bool,
    file_type: Option<&str>,
    compact: bool,
    extra_args: &[String],
    verbose: u8,
) -> Result<i32> {
    let timer = tracking::TimedExecution::start();
    debug_assert_eq!(classify_command("grep"), EvidenceSensitivity::RawDefault);

    if verbose > 0 {
        eprintln!("grep: '{}' in {}", pattern, path);
    }

    let rg_pattern = pattern.replace(r"\|", "|");

    let mut rg_cmd = resolved_command("rg");
    rg_cmd
        .args(["-n", "--no-heading", &rg_pattern, path])
        .stdin(Stdio::null());

    if let Some(ft) = file_type {
        rg_cmd.arg("--type").arg(ft);
    }

    for arg in extra_args {
        if arg == "-r" || arg == "--recursive" {
            continue;
        }
        rg_cmd.arg(arg);
    }

    let output = rg_cmd
        .output()
        .or_else(|_| {
            resolved_command("grep")
                .args(["-rn", pattern, path])
                .stdin(Stdio::null())
                .output()
        })
        .context("grep/rg failed")?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let exit_code = exit_code_from_output(&output, "grep");
    let raw_output = join_output_streams(&stdout, &stderr);
    let command_display = format!("grep -rn '{}' {}", pattern, path);

    if stdout.trim().is_empty() {
        if exit_code > 1 {
            print_raw_output(&stdout, &stderr);
            timer.track(&command_display, "rtk grep", &raw_output, &raw_output);
            return Ok(exit_code);
        }

        let msg = format!("0 matches for '{}'", pattern);
        println!("{}", msg);
        timer.track(&command_display, "rtk grep", &raw_output, &msg);
        return Ok(exit_code);
    }

    if !compact {
        print_raw_output(&stdout, &stderr);
        timer.track(&command_display, "rtk grep", &raw_output, &raw_output);
        return Ok(exit_code);
    }

    let compact_output = build_compact_output(
        &stdout,
        pattern,
        path,
        max_line_len,
        max_results,
        context_only,
    );
    let display = format!("[summary view]\n{}", compact_output);

    if let Some(hint) = crate::core::tee::force_tee_hint(&raw_output, "grep")
        .or_else(|| crate::core::tee::tee_and_hint(&raw_output, "grep", exit_code))
    {
        println!("{}\n{}", display, hint);
    } else {
        println!("{}", display);
    }

    timer.track(&command_display, "rtk grep", &raw_output, &display);
    Ok(exit_code)
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

fn build_compact_output(
    stdout: &str,
    pattern: &str,
    path: &str,
    max_line_len: usize,
    max_results: usize,
    context_only: bool,
) -> String {
    let mut by_file: HashMap<String, Vec<(usize, String)>> = HashMap::new();
    let mut total = 0;

    let context_re = if context_only {
        Regex::new(&format!("(?i).{{0,20}}{}.*", regex::escape(pattern))).ok()
    } else {
        None
    };

    for line in stdout.lines() {
        let parts: Vec<&str> = line.splitn(3, ':').collect();

        let (file, line_num, content) = if parts.len() == 3 {
            let ln = parts[1].parse().unwrap_or(0);
            (parts[0].to_string(), ln, parts[2])
        } else if parts.len() == 2 {
            let ln = parts[0].parse().unwrap_or(0);
            (path.to_string(), ln, parts[1])
        } else {
            continue;
        };

        total += 1;
        let cleaned = clean_line(content, max_line_len, context_re.as_ref(), pattern);
        by_file.entry(file).or_default().push((line_num, cleaned));
    }

    let mut rtk_output = String::new();
    rtk_output.push_str(&format!("{} matches in {} file(s)\n\n", total, by_file.len()));

    let mut shown = 0;
    let mut files: Vec<_> = by_file.iter().collect();
    files.sort_by_key(|(f, _)| *f);

    for (file, matches) in files {
        if shown >= max_results {
            break;
        }

        let file_display = compact_path(file);
        rtk_output.push_str(&format!("[file] {} ({})\n", file_display, matches.len()));

        let per_file = config::limits().grep_max_per_file;
        for (line_num, content) in matches.iter().take(per_file) {
            rtk_output.push_str(&format!("  {:>4}: {}\n", line_num, content));
            shown += 1;
            if shown >= max_results {
                break;
            }
        }

        if matches.len() > per_file {
            rtk_output.push_str(&format!("  +{} more hits in this file\n", matches.len() - per_file));
        }
        rtk_output.push('\n');
    }

    if total > shown {
        rtk_output.push_str(&format!("... +{} more matches\n", total - shown));
    }

    rtk_output.trim_end().to_string()
}

fn clean_line(line: &str, max_len: usize, context_re: Option<&Regex>, pattern: &str) -> String {
    let trimmed = line.trim();

    if let Some(re) = context_re {
        if let Some(m) = re.find(trimmed) {
            let matched = m.as_str();
            if matched.len() <= max_len {
                return matched.to_string();
            }
        }
    }

    if trimmed.len() <= max_len {
        trimmed.to_string()
    } else {
        let lower = trimmed.to_lowercase();
        let pattern_lower = pattern.to_lowercase();

        if let Some(pos) = lower.find(&pattern_lower) {
            let char_pos = lower[..pos].chars().count();
            let chars: Vec<char> = trimmed.chars().collect();
            let char_len = chars.len();

            let start = char_pos.saturating_sub(max_len / 3);
            let end = (start + max_len).min(char_len);
            let start = if end == char_len {
                end.saturating_sub(max_len)
            } else {
                start
            };

            let slice: String = chars[start..end].iter().collect();
            if start > 0 && end < char_len {
                format!("...{}...", slice)
            } else if start > 0 {
                format!("...{}", slice)
            } else {
                format!("{}...", slice)
            }
        } else {
            let t: String = trimmed.chars().take(max_len.saturating_sub(3)).collect();
            format!("{}...", t)
        }
    }
}

fn compact_path(path: &str) -> String {
    if path.len() <= 50 {
        return path.to_string();
    }

    let parts: Vec<&str> = path.split('/').collect();
    if parts.len() <= 3 {
        return path.to_string();
    }

    format!(
        "{}/.../{}/{}",
        parts[0],
        parts[parts.len() - 2],
        parts[parts.len() - 1]
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clean_line() {
        let line = "            const result = someFunction();";
        let cleaned = clean_line(line, 50, None, "result");
        assert!(!cleaned.starts_with(' '));
        assert!(cleaned.len() <= 50);
    }

    #[test]
    fn test_compact_path() {
        let path = "/Users/patrick/dev/project/src/components/Button.tsx";
        let compact = compact_path(path);
        assert!(compact.len() <= 60);
    }

    #[test]
    fn test_bre_alternation_translated() {
        let pattern = r"fn foo\|pub.*bar";
        let rg_pattern = pattern.replace(r"\|", "|");
        assert_eq!(rg_pattern, "fn foo|pub.*bar");
    }

    #[test]
    fn test_recursive_flag_stripped() {
        let extra_args: Vec<String> = vec!["-r".to_string(), "-i".to_string()];
        let filtered: Vec<&String> = extra_args
            .iter()
            .filter(|a| *a != "-r" && *a != "--recursive")
            .collect();
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0], "-i");
    }

    #[test]
    fn test_join_output_streams_preserves_stderr_boundary() {
        let joined = join_output_streams("a", "b");
        assert_eq!(joined, "a\nb");
    }

    #[test]
    fn test_grep_policy_is_raw_default() {
        assert_eq!(classify_command("grep"), EvidenceSensitivity::RawDefault);
    }
}
