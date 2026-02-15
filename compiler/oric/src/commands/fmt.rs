//! The `fmt` command: format Ori source files.
//!
//! Supports single files, directories, and stdin.
//! Uses parallel processing for directories when multiple files are found.

#![allow(
    clippy::struct_excessive_bools,
    reason = "FormatConfig has standard CLI config bool fields"
)]
#![allow(
    clippy::single_char_pattern,
    clippy::uninlined_format_args,
    clippy::format_in_format_args,
    clippy::manual_let_else,
    clippy::redundant_closure_for_method_calls,
    clippy::collapsible_else_if,
    reason = "CLI string processing — readability over micro-optimization in formatter"
)]

use ori_diagnostic::{span_utils, ErrorCode};

use crate::ir::StringInterner;
use crate::parser::ParseError;
use rayon::prelude::*;
use std::fmt::Write as _;
use std::io::{IsTerminal, Read};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use super::read_file;

/// Configuration for the format command.
#[derive(Default)]
pub struct FormatConfig {
    /// Check if files are formatted without modifying them.
    /// Returns exit code 1 if any files would be modified.
    pub check: bool,
    /// Show diff output instead of modifying files.
    pub diff: bool,
    /// Read from stdin and write to stdout.
    pub stdin: bool,
    /// Ignore .orifmtignore files and format everything.
    pub no_ignore: bool,
}

/// Result of formatting a single file.
pub enum FormatResult {
    /// File was unchanged (already formatted).
    Unchanged,
    /// File was formatted successfully.
    Formatted,
    /// File would be formatted (in check mode).
    WouldFormat,
    /// Parse error - file cannot be formatted.
    /// Contains the formatted error message.
    ParseError(String),
}

/// Format a single Ori source file.
///
/// Returns the format result indicating whether the file was changed.
pub fn format_file(path: &str, config: &FormatConfig) -> FormatResult {
    let content = read_file(path);
    format_content(path, &content, config)
}

/// Format content from stdin and write to stdout.
///
/// Returns true if the content was valid (no parse errors), false otherwise.
pub fn format_stdin() -> bool {
    let mut content = String::new();
    if let Err(e) = std::io::stdin().read_to_string(&mut content) {
        eprintln!("Error reading from stdin: {e}");
        return false;
    }

    let interner = StringInterner::new();

    // Preprocess: convert tabs to spaces
    let content = ori_fmt::tabs_to_spaces(&content);

    // Lex with comment preservation
    let lex_output = ori_lexer::lex_with_comments(&content, &interner);

    // Parse
    let parse_output = crate::parser::parse(&lex_output.tokens, &interner);
    if parse_output.has_errors() {
        let formatted_errors = format_parse_errors("<stdin>", &parse_output.errors, &content);
        eprint!("{formatted_errors}");
        return false;
    }

    // Format with comment preservation
    let formatted = ori_fmt::format_module_with_comments(
        &parse_output.module,
        &lex_output.comments,
        &parse_output.arena,
        &interner,
    );

    // Ensure trailing newline
    let formatted = if formatted.ends_with('\n') {
        formatted
    } else {
        format!("{formatted}\n")
    };

    // Write to stdout
    print!("{formatted}");

    true
}

/// Format content and optionally write to file.
fn format_content(path: &str, content: &str, config: &FormatConfig) -> FormatResult {
    let interner = StringInterner::new();

    // Preprocess: convert tabs to spaces
    let content = ori_fmt::tabs_to_spaces(content);

    // Lex with comment preservation
    let lex_output = ori_lexer::lex_with_comments(&content, &interner);

    // Parse
    let parse_output = crate::parser::parse(&lex_output.tokens, &interner);
    if parse_output.has_errors() {
        let formatted_errors = format_parse_errors(path, &parse_output.errors, &content);
        return FormatResult::ParseError(formatted_errors);
    }

    // Format with comment preservation
    let formatted = ori_fmt::format_module_with_comments(
        &parse_output.module,
        &lex_output.comments,
        &parse_output.arena,
        &interner,
    );

    // Ensure trailing newline
    let formatted = if formatted.ends_with('\n') {
        formatted
    } else {
        format!("{formatted}\n")
    };

    // Check if content changed
    if formatted == content {
        return FormatResult::Unchanged;
    }

    if config.check {
        return FormatResult::WouldFormat;
    }

    if config.diff {
        print_diff(path, &content, &formatted);
        return FormatResult::WouldFormat;
    }

    // Write the formatted content back
    if let Err(e) = std::fs::write(path, &formatted) {
        eprintln!("Error writing '{path}': {e}");
        std::process::exit(1);
    }

    FormatResult::Formatted
}

/// Print a unified diff between original and formatted content.
fn print_diff(path: &str, original: &str, formatted: &str) {
    println!("--- {path}");
    println!("+++ {path}");

    // Simple line-by-line diff
    let original_lines: Vec<&str> = original.lines().collect();
    let formatted_lines: Vec<&str> = formatted.lines().collect();

    let max_lines = original_lines.len().max(formatted_lines.len());

    // Find changed regions (very basic diff)
    let mut i = 0;
    while i < max_lines {
        let orig = original_lines.get(i);
        let fmt = formatted_lines.get(i);

        if orig == fmt {
            i += 1;
        } else {
            // Found a difference, print context
            let start = i.saturating_sub(2);
            let end = (i + 3).min(max_lines);

            println!(
                "@@ -{},{} +{},{} @@",
                start + 1,
                end - start,
                start + 1,
                end - start
            );

            for j in start..end {
                let o = original_lines.get(j);
                let f = formatted_lines.get(j);

                match (o, f) {
                    (Some(orig_line), Some(fmt_line)) if orig_line == fmt_line => {
                        println!(" {orig_line}");
                    }
                    (Some(orig_line), Some(fmt_line)) => {
                        println!("-{orig_line}");
                        println!("+{fmt_line}");
                    }
                    (Some(orig_line), None) => {
                        println!("-{orig_line}");
                    }
                    (None, Some(fmt_line)) => {
                        println!("+{fmt_line}");
                    }
                    (None, None) => {}
                }
            }

            // Skip past this changed region
            i = end;
        }
    }
}

/// Format all Ori files in a directory recursively.
///
/// Uses parallel processing for better performance on large directories.
pub fn format_directory(path: &str, config: &FormatConfig) -> (usize, usize, usize) {
    // Load ignore patterns from .orifmtignore files
    let ignore_patterns = if config.no_ignore {
        Vec::new()
    } else {
        load_ignore_patterns(Path::new(path))
    };

    // Collect all files first
    let mut files = Vec::new();
    visit_ori_files(
        Path::new(path),
        config,
        &ignore_patterns,
        &mut |file_path| {
            files.push(file_path.to_path_buf());
        },
    );

    // Use atomic counters for thread-safe counting
    let formatted_count = AtomicUsize::new(0);
    let unchanged_count = AtomicUsize::new(0);
    let error_count = AtomicUsize::new(0);

    // Process files in parallel
    files.par_iter().for_each(|file_path| {
        let path_str = file_path.display().to_string();
        match format_file(&path_str, config) {
            FormatResult::Formatted => {
                if !config.check && !config.diff {
                    println!("Formatted: {path_str}");
                }
                formatted_count.fetch_add(1, Ordering::Relaxed);
            }
            FormatResult::WouldFormat => {
                if config.check {
                    println!("Would format: {path_str}");
                }
                formatted_count.fetch_add(1, Ordering::Relaxed);
            }
            FormatResult::Unchanged => {
                unchanged_count.fetch_add(1, Ordering::Relaxed);
            }
            FormatResult::ParseError(errors) => {
                eprint!("{errors}");
                error_count.fetch_add(1, Ordering::Relaxed);
            }
        }
    });

    (
        formatted_count.load(Ordering::Relaxed),
        unchanged_count.load(Ordering::Relaxed),
        error_count.load(Ordering::Relaxed),
    )
}

/// Load ignore patterns from .orifmtignore file in the given directory.
fn load_ignore_patterns(root: &Path) -> Vec<String> {
    let ignore_file = root.join(".orifmtignore");
    if !ignore_file.exists() {
        return Vec::new();
    }

    match std::fs::read_to_string(&ignore_file) {
        Ok(content) => content
            .lines()
            .map(|line| line.trim())
            .filter(|line| !line.is_empty() && !line.starts_with('#'))
            .map(String::from)
            .collect(),
        Err(_) => Vec::new(),
    }
}

/// Check if a path matches any of the ignore patterns.
fn is_ignored(path: &Path, root: &Path, patterns: &[String]) -> bool {
    // Get the relative path from root
    let relative = match path.strip_prefix(root) {
        Ok(rel) => rel,
        Err(_) => return false,
    };

    let relative_str = relative.to_string_lossy();

    for pattern in patterns {
        // Simple glob matching: support * and ** patterns
        if pattern.contains("**") {
            // ** matches any number of directories
            let parts: Vec<&str> = pattern.split("**").collect();
            if parts.len() == 2 {
                let prefix = parts[0].trim_end_matches('/');
                let suffix = parts[1].trim_start_matches('/');

                // Check if path starts with prefix (if non-empty)
                let matches_prefix = prefix.is_empty()
                    || relative_str.starts_with(prefix)
                    || relative_str.starts_with(&format!("{prefix}/"));

                // Check if path ends with suffix (if non-empty)
                let matches_suffix = suffix.is_empty() || relative_str.ends_with(suffix);

                if matches_prefix && matches_suffix {
                    return true;
                }
            }
        } else if pattern.contains('*') {
            // Single * matches anything except /
            let parts: Vec<&str> = pattern.split('*').collect();
            if parts.len() == 2 {
                let prefix = parts[0];
                let suffix = parts[1];

                if relative_str.starts_with(prefix)
                    && relative_str.ends_with(suffix)
                    && !relative_str[prefix.len()..relative_str.len() - suffix.len()].contains('/')
                {
                    return true;
                }
            }
        } else {
            // Exact match or directory match
            if relative_str == *pattern || relative_str.starts_with(&format!("{pattern}/")) {
                return true;
            }

            // Also check if the file name matches (for patterns like "foo.ori")
            if let Some(file_name) = path.file_name().and_then(|n| n.to_str()) {
                if file_name == pattern {
                    return true;
                }
            }
        }
    }

    false
}

/// Visit all .ori files in a directory recursively.
fn visit_ori_files<F: FnMut(&Path)>(
    dir: &Path,
    config: &FormatConfig,
    ignore_patterns: &[String],
    callback: &mut F,
) {
    // Use the original dir as the root for relative path calculations
    visit_ori_files_impl(dir, dir, config, ignore_patterns, callback);
}

fn visit_ori_files_impl<F: FnMut(&Path)>(
    dir: &Path,
    root: &Path,
    config: &FormatConfig,
    ignore_patterns: &[String],
    callback: &mut F,
) {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(e) => {
            eprintln!("Error reading directory '{}': {e}", dir.display());
            return;
        }
    };

    for entry in entries.flatten() {
        let path = entry.path();

        // Skip hidden files and common ignored directories (unless --no-ignore)
        if !config.no_ignore {
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                if name.starts_with('.') || name == "target" || name == "node_modules" {
                    continue;
                }
            }

            // Check custom ignore patterns
            if is_ignored(&path, root, ignore_patterns) {
                continue;
            }
        }

        if path.is_dir() {
            visit_ori_files_impl(&path, root, config, ignore_patterns, callback);
        } else if path.extension().is_some_and(|ext| ext == "ori") {
            callback(&path);
        }
    }
}

/// Run the format command.
pub fn run_format(args: &[String]) {
    let mut config = FormatConfig::default();
    let mut paths: Vec<String> = Vec::new();

    // Parse arguments
    for arg in args {
        match arg.as_str() {
            "--check" => config.check = true,
            "--diff" => config.diff = true,
            "--stdin" => config.stdin = true,
            "--no-ignore" => config.no_ignore = true,
            "--help" | "-h" => {
                print_fmt_help();
                return;
            }
            arg if arg.starts_with('-') => {
                eprintln!("Unknown option: {arg}");
                eprintln!("Run 'ori fmt --help' for usage");
                std::process::exit(1);
            }
            _ => paths.push(arg.clone()),
        }
    }

    // Handle stdin mode
    if config.stdin {
        if !paths.is_empty() {
            eprintln!("Cannot specify paths with --stdin");
            std::process::exit(1);
        }
        if config.check {
            eprintln!("Cannot use --check with --stdin");
            std::process::exit(1);
        }
        if config.diff {
            eprintln!("Cannot use --diff with --stdin");
            std::process::exit(1);
        }
        if !format_stdin() {
            std::process::exit(1);
        }
        return;
    }

    // Default to current directory if no paths specified
    if paths.is_empty() {
        paths.push(".".to_string());
    }

    let mut total_formatted = 0;
    let mut total_unchanged = 0;
    let mut total_errors = 0;

    for path in &paths {
        let path_obj = PathBuf::from(path);

        if path_obj.is_file() {
            match format_file(path, &config) {
                FormatResult::Formatted => {
                    if !config.check && !config.diff {
                        println!("Formatted: {path}");
                    }
                    total_formatted += 1;
                }
                FormatResult::WouldFormat => {
                    if config.check {
                        println!("Would format: {path}");
                    }
                    total_formatted += 1;
                }
                FormatResult::Unchanged => {
                    total_unchanged += 1;
                }
                FormatResult::ParseError(errors) => {
                    eprint!("{errors}");
                    total_errors += 1;
                }
            }
        } else if path_obj.is_dir() {
            let (formatted, unchanged, errors) = format_directory(path, &config);
            total_formatted += formatted;
            total_unchanged += unchanged;
            total_errors += errors;
        } else {
            eprintln!("Path not found: {path}");
            total_errors += 1;
        }
    }

    // Print summary for directory operations
    if paths.len() > 1 || paths.iter().any(|p| PathBuf::from(p).is_dir()) {
        let verb = if config.check {
            "would format"
        } else {
            "formatted"
        };
        if total_formatted > 0 || total_unchanged > 0 {
            println!("\n{total_formatted} {verb}, {total_unchanged} unchanged");
        }
    }

    // Exit with error code if check mode found unformatted files
    if config.check && total_formatted > 0 {
        std::process::exit(1);
    }

    // Exit with error code if there were parse errors
    if total_errors > 0 {
        std::process::exit(1);
    }
}

fn print_fmt_help() {
    println!("Format Ori source files");
    println!();
    println!("Usage: ori fmt [options] [paths...]");
    println!();
    println!("Arguments:");
    println!("  paths        Files or directories to format (default: .)");
    println!();
    println!("Options:");
    println!("  --check      Check if files are formatted (exit 1 if not)");
    println!("  --diff       Show diff output instead of modifying files");
    println!("  --stdin      Read from stdin, write to stdout");
    println!("  --no-ignore  Ignore .orifmtignore files and format everything");
    println!("  --help       Show this help message");
    println!();
    println!("Ignore files:");
    println!("  Create a .orifmtignore file to exclude paths from formatting.");
    println!("  Patterns support * (single directory) and ** (any directories).");
    println!("  Default ignores: hidden files (.*), target/, node_modules/");
    println!();
    println!("Examples:");
    println!("  ori fmt                    # Format all files in current directory");
    println!("  ori fmt src/               # Format all files in src/");
    println!("  ori fmt main.ori           # Format a single file");
    println!("  ori fmt --check            # Check formatting in CI");
    println!("  ori fmt --diff main.ori    # Preview formatting changes");
    println!("  cat main.ori | ori fmt --stdin   # Format stdin to stdout");
    println!("  ori fmt --no-ignore        # Format everything (ignore .orifmtignore)");
}

/// ANSI color codes for terminal output.
mod colors {
    pub const ERROR: &str = "\x1b[1;31m"; // Bold red
    pub const NOTE: &str = "\x1b[1;36m"; // Bold cyan
    pub const HELP: &str = "\x1b[1;32m"; // Bold green
    pub const BOLD: &str = "\x1b[1m";
    pub const BLUE: &str = "\x1b[1;34m"; // Bold blue
    pub const RESET: &str = "\x1b[0m";
}

/// Check if stderr is a terminal (for color output).
fn use_colors() -> bool {
    std::io::stderr().is_terminal()
}

/// Get the source line containing the given byte offset.
fn get_source_line(source: &str, offset: u32) -> Option<(&str, usize)> {
    let offset = offset as usize;
    if offset > source.len() {
        return None;
    }

    // Find line start
    let line_start = source[..offset].rfind('\n').map_or(0, |pos| pos + 1);

    // Find line end
    let line_end = source[offset..]
        .find('\n')
        .map_or(source.len(), |pos| offset + pos);

    let line = &source[line_start..line_end];
    Some((line, line_start))
}

/// Generate a suggestion for common formatting errors.
fn get_suggestion(error: &ParseError) -> Option<String> {
    let msg = error.message();
    let ctx = error.context().unwrap_or("");

    // Suggestions based on error code
    match error.code() {
        ErrorCode::E1003 => {
            // Unclosed delimiter
            Some("check for missing closing bracket, parenthesis, or brace".to_string())
        }
        ErrorCode::E1001 => {
            // Unexpected token - check for common mistakes in message and context
            if msg.contains("expected )") || msg.contains("expected `)") || ctx.contains(")") {
                Some("check for missing closing parenthesis or comma".to_string())
            } else if msg.contains("expected }") || msg.contains("expected `}") || ctx.contains("}")
            {
                Some("check for missing closing brace".to_string())
            } else if msg.contains("expected ]") || msg.contains("expected `]") || ctx.contains("]")
            {
                Some("check for missing closing bracket".to_string())
            } else if msg.contains("expected =")
                || msg.contains("expected `=`")
                || ctx.contains("=")
            {
                Some("function definitions require `=` before the body".to_string())
            } else if msg.contains("expected ,") || msg.contains("expected `,") || ctx.contains(",")
            {
                Some("check for missing comma or colon between items".to_string())
            } else if msg.contains("expected :")
                || msg.contains("expected `:`")
                || ctx.contains(":")
            {
                Some("parameter types use: name: Type".to_string())
            } else {
                None
            }
        }
        ErrorCode::E1002 => {
            // Expected expression
            Some("an expression is required here".to_string())
        }
        ErrorCode::E1004 => {
            // Expected identifier
            Some("identifiers must start with a letter or underscore".to_string())
        }
        ErrorCode::E1005 => {
            // Expected type
            Some("type annotations use: name: Type".to_string())
        }
        ErrorCode::E1006 => {
            // Invalid function definition
            Some("function definitions use: @name (params) -> ReturnType = body".to_string())
        }
        ErrorCode::E1007 => {
            // Missing function body
            Some("add `= expression` after the function signature".to_string())
        }
        ErrorCode::E1011 => {
            // Multi-arg function call requires named arguments
            Some("use named arguments: func(arg1: val1, arg2: val2)".to_string())
        }
        ErrorCode::E0001 => {
            // Unterminated string
            Some("add a closing `\"` to terminate the string".to_string())
        }
        ErrorCode::E0004 => {
            // Unterminated character literal
            Some("character literals use single quotes: 'a'".to_string())
        }
        ErrorCode::E0005 => {
            // Invalid escape sequence
            Some("valid escape sequences: \\n, \\t, \\r, \\\\, \\\", \\'".to_string())
        }
        _ => {
            // Check error context for additional hints
            if ctx.contains("attribute") {
                return Some("attributes use: #name(args)".to_string());
            }
            // Check message for patterns
            if msg.contains("expected identifier") {
                return Some("identifiers must start with a letter or underscore".to_string());
            }
            if msg.contains("expected type") {
                return Some("type annotations use: name: Type".to_string());
            }
            None
        }
    }
}

/// Format a parse error with source code snippet.
fn format_parse_error(path: &str, error: &ParseError, source: &str) -> String {
    let use_color = use_colors();
    let mut output = String::new();

    // Get line/column information
    let span = error.span();
    let (line, col) = span_utils::offset_to_line_col(source, span.start);
    let end_col = if span.end > span.start {
        let (_, ec) = span_utils::offset_to_line_col(source, span.end);
        ec
    } else {
        col + 1
    };

    // Error header
    if use_color {
        let _ = writeln!(
            output,
            "{}error{}{}{}: {}",
            colors::ERROR,
            colors::RESET,
            colors::BOLD,
            format!("[{}]", error.code()),
            error.message()
        );
    } else {
        let _ = writeln!(output, "error[{}]: {}", error.code(), error.message());
    }

    // File location
    if use_color {
        let _ = writeln!(
            output,
            "  {}-->{} {}:{}:{}",
            colors::BLUE,
            colors::RESET,
            path,
            line,
            col
        );
    } else {
        let _ = writeln!(output, "  --> {}:{}:{}", path, line, col);
    }

    // Source line with line number
    if let Some((source_line, _)) = get_source_line(source, span.start) {
        let line_num_str = line.to_string();
        let padding = " ".repeat(line_num_str.len());

        // Empty line before source
        if use_color {
            let _ = writeln!(output, "  {} {}|{}", padding, colors::BLUE, colors::RESET);
        } else {
            let _ = writeln!(output, "  {} |", padding);
        }

        // Source line
        if use_color {
            let _ = writeln!(
                output,
                "  {}{} |{} {}",
                colors::BLUE,
                line_num_str,
                colors::RESET,
                source_line
            );
        } else {
            let _ = writeln!(output, "  {} | {}", line_num_str, source_line);
        }

        // Underline
        let underline_start = col.saturating_sub(1) as usize;
        let underline_len = (end_col.saturating_sub(col) as usize).max(1);
        let underline_padding = " ".repeat(underline_start);
        let underline = "^".repeat(underline_len);

        if use_color {
            let _ = writeln!(
                output,
                "  {} {}|{} {}{}{}{}",
                padding,
                colors::BLUE,
                colors::RESET,
                underline_padding,
                colors::ERROR,
                underline,
                colors::RESET
            );
        } else {
            let _ = writeln!(output, "  {} | {}{}", padding, underline_padding, underline);
        }
    }

    // Suggestion
    if let Some(suggestion) = get_suggestion(error) {
        if use_color {
            let _ = writeln!(
                output,
                "  = {}help{}: {}",
                colors::HELP,
                colors::RESET,
                suggestion
            );
        } else {
            let _ = writeln!(output, "  = help: {}", suggestion);
        }
    }

    // Context (if available)
    if let Some(ctx) = error.context() {
        if use_color {
            let _ = writeln!(output, "  = {}note{}: {}", colors::NOTE, colors::RESET, ctx);
        } else {
            let _ = writeln!(output, "  = note: {}", ctx);
        }
    }

    output
}

/// Format multiple parse errors with source code snippets.
fn format_parse_errors(path: &str, errors: &[ParseError], source: &str) -> String {
    let use_color = use_colors();
    let mut output = String::new();

    for error in errors {
        output.push_str(&format_parse_error(path, error, source));
        output.push('\n');
    }

    // Add a summary note for multiple errors or a hint about formatting
    if errors.len() == 1 {
        if use_color {
            let _ = writeln!(
                output,
                "{}note{}: fix the syntax error to enable formatting",
                colors::NOTE,
                colors::RESET
            );
        } else {
            let _ = writeln!(output, "note: fix the syntax error to enable formatting");
        }
    } else {
        if use_color {
            let _ = writeln!(
                output,
                "{}note{}: fix {} syntax errors to enable formatting",
                colors::NOTE,
                colors::RESET,
                errors.len()
            );
        } else {
            let _ = writeln!(
                output,
                "note: fix {} syntax errors to enable formatting",
                errors.len()
            );
        }
    }

    output
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "tests panic on unexpected state for clear failure messages"
)]
mod tests;
