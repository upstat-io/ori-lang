//! Directive parsing for test files.
//!
//! Parses `// @<key>: <value>` and `// CHECK:` style directives from test
//! source files using line-anchored regex. Does NOT use the Ori lexer —
//! the harness must not depend on any compiler crate.
//!
//! **Limitation**: Line-based parsing only. Cannot handle multi-line
//! directives or directives inside block comments. This is acceptable —
//! Rust's compiletest has the same limitation.

use std::sync::LazyLock;

use regex::Regex;

#[cfg(test)]
mod tests;

// Types

/// A parsed directive from a test file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Directive {
    /// `// @revisions: debug release no-repr-opt`
    Revisions { names: Vec<String> },
    /// `// @compile-flags: --release`
    CompileFlags { flags: Vec<String> },
    /// `// CHECK: <pattern>`
    Check { pattern: String },
    /// `// CHECK-LABEL: <pattern>`
    CheckLabel { pattern: String },
    /// `// CHECK-NOT: <pattern>`
    CheckNot { pattern: String },
    /// `// CHECK-NEXT: <pattern>`
    CheckNext { pattern: String },
    /// `// @<key>: <value>` — consumer-specific directive.
    /// The harness parses the `key: value` structure; interpretation
    /// is delegated to the consumer's `TestStrategy`.
    Custom { key: String, value: String },
}

/// A directive line with source location and revision gate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirectiveLine {
    pub line_number: usize,
    pub revision: Option<String>,
    pub directive: Directive,
}

/// An error encountered during directive parsing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    pub line_number: usize,
    pub message: String,
}

/// Result of parsing directives from a test file.
#[derive(Debug)]
pub struct ParseResult {
    pub directives: Vec<DirectiveLine>,
    pub errors: Vec<ParseError>,
}

// Forbidden revision names (from Rust compiletest)

const FORBIDDEN_REVISION_NAMES: &[&str] = &[
    "true", "false", "CHECK", "COM", "NEXT", "SAME", "EMPTY", "NOT", "COUNT", "DAG", "LABEL",
];

// Regex patterns (compiled once via LazyLock)

/// Matches `// @[revision] key: value` or `// @key: value`
static RE_AT_DIRECTIVE: LazyLock<Regex> = LazyLock::new(|| {
    // SAFETY: regex is a compile-time constant; panic is a programming error
    #[expect(clippy::expect_used, reason = "compile-time constant regex")]
    Regex::new(r"^\s*//\s*@(?:\[([^\]]+)\]\s*)?(\S+?):\s*(.*)$").expect("directive regex")
});

/// Matches `// @` prefix lines that aren't valid directives — used to detect
/// malformed directives (recognized prefix but unparseable structure).
static RE_AT_PREFIX: LazyLock<Regex> = LazyLock::new(|| {
    #[expect(clippy::expect_used, reason = "compile-time constant regex")]
    Regex::new(r"^\s*//\s*@(?:\[([^\]]+)\])?\s*\S").expect("at-prefix regex")
});

/// Matches near-miss CHECK lines — `// CHECK` with optional `-SUFFIX` but
/// missing/wrong colon, or common typos (`CHEKC`, `CHCK`, `CEHCK`). Uses
/// word boundary (`\b`) after CHECK to avoid matching non-directives like
/// `// CHECKPOINT:` or `// CHECKED:`.
static RE_CHECK_NEAR_MISS: LazyLock<Regex> = LazyLock::new(|| {
    #[expect(clippy::expect_used, reason = "compile-time constant regex")]
    // (1) CHECK with optional -SUFFIX and word boundary — catches `// CHECK foo`
    //     and `// CHECK-TYPO:` but NOT `// CHECKPOINT:` or `// CHECKED:`.
    // (2) Common typos: CHEKC, CHCK, CEHCK (always near-miss regardless of suffix)
    Regex::new(r"^\s*//\s*(?:@\[([^\]]+)\]\s*)?(?:CHECK(?:-\w+)?\b.*|(?:CHEKC|CHCK|CEHCK).*)$")
        .expect("check near-miss regex")
});

/// Matches `// CHECK:`, `// CHECK-LABEL:`, `// CHECK-NOT:`, `// CHECK-NEXT:`
/// with optional `[revision]` prefix.
static RE_CHECK_DIRECTIVE: LazyLock<Regex> = LazyLock::new(|| {
    #[expect(clippy::expect_used, reason = "compile-time constant regex")]
    Regex::new(r"^\s*//\s*(?:@\[([^\]]+)\]\s*)?CHECK(-LABEL|-NOT|-NEXT)?:\s*(.*)$")
        .expect("check directive regex")
});

// Parsing

/// Parse directives from a test file's source text.
///
/// Returns both successfully parsed directives and any parse errors
/// (forbidden revision names, malformed directives). Line numbers are
/// 1-based.
pub fn parse_directives(source: &str) -> ParseResult {
    let mut directives = Vec::new();
    let mut errors = Vec::new();
    let mut seen_revisions = false;

    for (idx, line) in source.lines().enumerate() {
        let line_number = idx + 1;

        // Try CHECK-style directives first (they don't have the @ prefix)
        if let Some(caps) = RE_CHECK_DIRECTIVE.captures(line) {
            let revision = caps.get(1).map(|m| m.as_str().to_string());
            let suffix = caps.get(2).map(|m| m.as_str());
            let pattern = caps[3].trim().to_string();

            let directive = match suffix {
                None => Directive::Check { pattern },
                Some("-LABEL") => Directive::CheckLabel { pattern },
                Some("-NOT") => Directive::CheckNot { pattern },
                Some("-NEXT") => Directive::CheckNext { pattern },
                _ => {
                    errors.push(ParseError {
                        line_number,
                        message: format!("unknown CHECK suffix: {}", suffix.unwrap_or("")),
                    });
                    continue;
                }
            };

            if let Some(ref rev) = revision {
                if let Some(err) = validate_revision_name(rev, line_number) {
                    errors.push(err);
                    continue;
                }
            }

            directives.push(DirectiveLine {
                line_number,
                revision,
                directive,
            });
            continue;
        }

        // Detect near-miss CHECK lines (typos like "// CHEKC:" or "// CHECK foo")
        if RE_CHECK_NEAR_MISS.is_match(line) {
            errors.push(ParseError {
                line_number,
                message: format!(
                    "malformed CHECK directive (missing colon or typo?): {}",
                    line.trim()
                ),
            });
            continue;
        }

        // Try @-style directives
        if let Some(caps) = RE_AT_DIRECTIVE.captures(line) {
            if let Some(result) =
                parse_at_directive(&caps, line_number, &mut seen_revisions, &mut errors)
            {
                directives.push(result);
            }
            continue;
        }

        // Detect malformed directives: lines with `// @` prefix that didn't
        // match a valid directive pattern (recognized prefix, unparseable value).
        if RE_AT_PREFIX.is_match(line) {
            errors.push(ParseError {
                line_number,
                message: format!(
                    "malformed directive (expected `// @key: value`): {}",
                    line.trim()
                ),
            });
        }
    }

    ParseResult { directives, errors }
}

/// Parse a `// @key: value` directive from a regex capture.
fn parse_at_directive(
    caps: &regex::Captures<'_>,
    line_number: usize,
    seen_revisions: &mut bool,
    errors: &mut Vec<ParseError>,
) -> Option<DirectiveLine> {
    let revision = caps.get(1).map(|m| m.as_str().to_string());
    let key = caps[2].to_string();
    let value = caps[3].trim().to_string();

    if let Some(ref rev) = revision {
        if let Some(err) = validate_revision_name(rev, line_number) {
            errors.push(err);
            return None;
        }
    }

    let directive = match key.as_str() {
        "revisions" => {
            if *seen_revisions {
                errors.push(ParseError {
                    line_number,
                    message: "duplicate revisions directive (only one allowed per file)"
                        .to_string(),
                });
                return None;
            }
            *seen_revisions = true;
            let names: Vec<String> = value.split_whitespace().map(String::from).collect();
            if names.is_empty() {
                errors.push(ParseError {
                    line_number,
                    message: "empty revisions list (expected at least one name)".to_string(),
                });
                return None;
            }
            for name in &names {
                if let Some(err) = validate_revision_name(name, line_number) {
                    errors.push(err);
                    return None;
                }
            }
            Directive::Revisions { names }
        }
        "compile-flags" => Directive::CompileFlags {
            flags: value.split_whitespace().map(String::from).collect(),
        },
        _ => Directive::Custom { key, value },
    };

    Some(DirectiveLine {
        line_number,
        revision,
        directive,
    })
}

fn validate_revision_name(name: &str, line_number: usize) -> Option<ParseError> {
    if FORBIDDEN_REVISION_NAMES
        .iter()
        .any(|&forbidden| forbidden.eq_ignore_ascii_case(name))
    {
        Some(ParseError {
            line_number,
            message: format!("forbidden revision name: '{name}'"),
        })
    } else {
        None
    }
}
