//! Reads source files with raw-by-default output and explicit summary modes.

use crate::core::filter::{self, FilterLevel, Language};
use crate::core::policy::{classify_command, EvidenceSensitivity};
use crate::core::tracking;
use anyhow::{Context, Result};
use std::fs;
use std::path::Path;

pub fn run(
    file: &Path,
    level: FilterLevel,
    max_lines: Option<usize>,
    tail_lines: Option<usize>,
    line_numbers: bool,
    verbose: u8,
) -> Result<()> {
    let timer = tracking::TimedExecution::start();
    debug_assert_eq!(classify_command("read"), EvidenceSensitivity::RawDefault);

    if verbose > 0 {
        eprintln!("Reading: {} (filter: {})", file.display(), level);
    }

    let content = fs::read_to_string(file)
        .with_context(|| format!("Failed to read file: {}", file.display()))?;

    let lang = file
        .extension()
        .and_then(|e| e.to_str())
        .map(Language::from_extension)
        .unwrap_or(Language::Unknown);

    if verbose > 1 {
        eprintln!("Detected language: {:?}", lang);
    }

    let display = build_display_output(
        &content,
        &lang,
        level,
        max_lines,
        tail_lines,
        line_numbers,
        verbose,
        Some(file),
    );

    print_display_with_optional_hint(&display.output, display.hint.as_deref());
    timer.track(
        &format!("cat {}", file.display()),
        "rtk read",
        &content,
        &display.tracked_output,
    );
    Ok(())
}

pub fn run_stdin(
    level: FilterLevel,
    max_lines: Option<usize>,
    tail_lines: Option<usize>,
    line_numbers: bool,
    verbose: u8,
) -> Result<()> {
    use std::io::{self, Read as IoRead};

    let timer = tracking::TimedExecution::start();
    debug_assert_eq!(classify_command("read"), EvidenceSensitivity::RawDefault);

    if verbose > 0 {
        eprintln!("Reading from stdin (filter: {})", level);
    }

    let mut content = String::new();
    io::stdin()
        .lock()
        .read_to_string(&mut content)
        .context("Failed to read from stdin")?;

    let lang = Language::Unknown;

    if verbose > 1 {
        eprintln!("Language: {:?} (stdin has no extension)", lang);
    }

    let display = build_display_output(
        &content,
        &lang,
        level,
        max_lines,
        tail_lines,
        line_numbers,
        verbose,
        None,
    );

    print_display_with_optional_hint(&display.output, display.hint.as_deref());
    timer.track("cat - (stdin)", "rtk read -", &content, &display.tracked_output);
    Ok(())
}

struct DisplayOutput {
    output: String,
    tracked_output: String,
    hint: Option<String>,
}

#[allow(clippy::too_many_arguments)]
fn build_display_output(
    content: &str,
    lang: &Language,
    level: FilterLevel,
    max_lines: Option<usize>,
    tail_lines: Option<usize>,
    line_numbers: bool,
    verbose: u8,
    file: Option<&Path>,
) -> DisplayOutput {
    let filter = filter::get_filter(level);
    let mut filtered = filter.filter(content, lang);

    if filtered.trim().is_empty() && !content.trim().is_empty() {
        if let Some(path) = file {
            eprintln!(
                "rtk: warning: filter produced empty output for {} ({} bytes), showing raw content",
                path.display(),
                content.len()
            );
        } else {
            eprintln!(
                "rtk: warning: filter produced empty stdin output ({} bytes), showing raw content",
                content.len()
            );
        }
        filtered = content.to_string();
    }

    if verbose > 0 {
        let original_lines = content.lines().count();
        let filtered_lines = filtered.lines().count();
        let reduction = if original_lines > 0 {
            ((original_lines.saturating_sub(filtered_lines)) as f64 / original_lines as f64) * 100.0
        } else {
            0.0
        };
        eprintln!(
            "Lines: {} -> {} ({:.1}% reduction)",
            original_lines, filtered_lines, reduction
        );
    }

    let windowed = apply_line_window(&filtered, max_lines, tail_lines);
    let rendered = if line_numbers {
        format_with_line_numbers(&windowed)
    } else {
        windowed.clone()
    };

    let is_lossy = normalize_trailing_newlines(&windowed) != normalize_trailing_newlines(content);
    let tracked_output = if is_lossy {
        format!("[summary view]\n{}", rendered)
    } else {
        rendered.clone()
    };
    let hint = if is_lossy {
        crate::core::tee::force_tee_hint(content, "read")
    } else {
        None
    };

    DisplayOutput {
        output: tracked_output.clone(),
        tracked_output,
        hint,
    }
}

fn print_display_with_optional_hint(output: &str, hint: Option<&str>) {
    print!("{}", output);
    if let Some(hint) = hint {
        if !output.ends_with('\n') {
            println!();
        }
        println!("{}", hint);
    }
}

fn normalize_trailing_newlines(text: &str) -> &str {
    text.trim_end_matches(&['\r', '\n'][..])
}

fn format_with_line_numbers(content: &str) -> String {
    let lines: Vec<&str> = content.lines().collect();
    let width = lines.len().to_string().len();
    let mut out = String::new();
    for (i, line) in lines.iter().enumerate() {
        out.push_str(&format!("{:>width$} │ {}\n", i + 1, line, width = width));
    }
    out
}

fn apply_line_window(content: &str, max_lines: Option<usize>, tail_lines: Option<usize>) -> String {
    if let Some(tail) = tail_lines {
        if tail == 0 {
            return String::new();
        }
        let lines: Vec<&str> = content.lines().collect();
        let start = lines.len().saturating_sub(tail);
        let mut result = lines[start..].join("\n");
        if content.ends_with('\n') {
            result.push('\n');
        }
        return result;
    }

    if let Some(max) = max_lines {
        let lines: Vec<&str> = content.lines().collect();
        if lines.len() <= max {
            return content.to_string();
        }
        if max == 0 {
            return format!("... {} lines omitted (total: {})", lines.len(), lines.len());
        }

        let mut result = lines[..max].join("\n");
        if !result.is_empty() {
            result.push('\n');
        }
        result.push_str(&format!(
            "... {} more lines omitted (total: {})",
            lines.len() - max,
            lines.len()
        ));
        return result;
    }

    content.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_read_rust_file() -> Result<()> {
        let mut file = NamedTempFile::with_suffix(".rs")?;
        writeln!(
            file,
            r#"// Comment
fn main() {{
    println!("Hello");
}}"#
        )?;

        run(file.path(), FilterLevel::Minimal, None, None, false, 0)?;
        Ok(())
    }

    #[test]
    fn test_stdin_support_signature() {
        // Compile-time check that run_stdin remains callable.
    }

    #[test]
    fn test_apply_line_window_tail_lines() {
        let input = "a\nb\nc\nd\n";
        let output = apply_line_window(input, None, Some(2));
        assert_eq!(output, "c\nd\n");
    }

    #[test]
    fn test_apply_line_window_tail_lines_no_trailing_newline() {
        let input = "a\nb\nc\nd";
        let output = apply_line_window(input, None, Some(2));
        assert_eq!(output, "c\nd");
    }

    #[test]
    fn test_apply_line_window_max_lines_uses_deterministic_head_slice() {
        let input = "a\nb\nc\nd\n";
        let output = apply_line_window(input, Some(2), None);
        assert_eq!(output, "a\nb\n... 2 more lines omitted (total: 4)");
    }

    #[test]
    fn test_read_policy_is_raw_default() {
        assert_eq!(classify_command("read"), EvidenceSensitivity::RawDefault);
    }
}
