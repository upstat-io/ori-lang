//! `FileCheck`-style matching engine.
//!
//! Supports two modes:
//! - `.matches` (default): order-independent substring matching.
//!   Every CHECK pattern must appear somewhere in the IR, but order
//!   between CHECK lines is not enforced. CHECK-NOT patterns must
//!   NOT appear anywhere.
//! - `.exact`: sequential matching (traditional `FileCheck` behavior).
//!   CHECK patterns must appear in the order specified. CHECK-NEXT
//!   requires the match to be on the immediately following line.

use crate::directive::{Directive, DirectiveLine};

/// Matching mode for CHECK directives.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CheckMode {
    /// Order-independent substring matching (default, Zig pattern).
    Matches,
    /// Sequential matching (traditional `FileCheck`).
    Exact,
}

/// Result of running CHECK directive matching.
#[derive(Clone, Debug)]
pub struct CheckResult {
    /// Whether all checks passed.
    pub passed: bool,
    /// Details of each failure.
    pub failures: Vec<CheckFailure>,
}

/// A single CHECK directive failure.
#[derive(Clone, Debug)]
pub enum CheckFailure {
    /// A `CHECK:` pattern was not found in the IR.
    PatternNotFound { source_line: usize, pattern: String },
    /// A `CHECK-NOT:` pattern was unexpectedly found.
    NegativePatternFound {
        source_line: usize,
        pattern: String,
        found_at_line: usize,
    },
    /// A `CHECK-LABEL:` pattern was not found (section anchor missing).
    LabelNotFound { source_line: usize, pattern: String },
    /// A `CHECK-NEXT:` pattern was not on the next line after the previous match.
    NextNotAdjacent {
        source_line: usize,
        pattern: String,
        expected_line: usize,
        actual_line: usize,
    },
}

impl std::fmt::Display for CheckFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::PatternNotFound {
                source_line,
                pattern,
            } => write!(f, "line {source_line}: CHECK pattern not found: `{pattern}`"),
            Self::NegativePatternFound {
                source_line,
                pattern,
                found_at_line,
            } => write!(
                f,
                "line {source_line}: CHECK-NOT pattern found at IR line {found_at_line}: `{pattern}`"
            ),
            Self::LabelNotFound {
                source_line,
                pattern,
            } => write!(
                f,
                "line {source_line}: CHECK-LABEL pattern not found: `{pattern}`"
            ),
            Self::NextNotAdjacent {
                source_line,
                pattern,
                expected_line,
                actual_line,
            } => write!(
                f,
                "line {source_line}: CHECK-NEXT expected at line {expected_line}, \
                 found at line {actual_line}: `{pattern}`"
            ),
        }
    }
}

/// Run CHECK directives against IR output.
///
/// Only processes `Check`, `CheckNot`, `CheckLabel`, and `CheckNext` directives;
/// other directive types (revisions, compile-flags) are skipped.
pub fn run_checks(ir: &str, directives: &[DirectiveLine], mode: CheckMode) -> CheckResult {
    match mode {
        CheckMode::Matches => run_matches_mode(ir, directives),
        CheckMode::Exact => run_exact_mode(ir, directives),
    }
}

/// Order-independent substring matching.
///
/// Every CHECK pattern must appear somewhere in the IR. CHECK-NOT patterns
/// must NOT appear anywhere. CHECK-LABEL and CHECK-NEXT are not meaningful
/// in matches mode — CHECK-LABEL is treated as CHECK, CHECK-NEXT is treated
/// as CHECK (order is not enforced).
fn run_matches_mode(ir: &str, directives: &[DirectiveLine]) -> CheckResult {
    let ir_lines: Vec<&str> = ir.lines().collect();
    let mut failures = Vec::new();

    for dl in directives {
        match &dl.directive {
            Directive::Check { pattern } | Directive::CheckLabel { pattern } => {
                if !ir_lines.iter().any(|line| line.contains(pattern.as_str())) {
                    failures.push(CheckFailure::PatternNotFound {
                        source_line: dl.line_number,
                        pattern: pattern.clone(),
                    });
                }
            }
            Directive::CheckNot { pattern } => {
                for (i, line) in ir_lines.iter().enumerate() {
                    if line.contains(pattern.as_str()) {
                        failures.push(CheckFailure::NegativePatternFound {
                            source_line: dl.line_number,
                            pattern: pattern.clone(),
                            found_at_line: i + 1,
                        });
                        break;
                    }
                }
            }
            Directive::CheckNext { pattern } => {
                // In matches mode, CHECK-NEXT is treated as CHECK
                if !ir_lines.iter().any(|line| line.contains(pattern.as_str())) {
                    failures.push(CheckFailure::PatternNotFound {
                        source_line: dl.line_number,
                        pattern: pattern.clone(),
                    });
                }
            }
            // Skip non-check directives
            _ => {}
        }
    }

    CheckResult {
        passed: failures.is_empty(),
        failures,
    }
}

/// Sequential matching (traditional `FileCheck`).
///
/// CHECK patterns must appear in the order specified. CHECK-NEXT requires
/// the match to be on the immediately following line after the previous match.
fn run_exact_mode(ir: &str, directives: &[DirectiveLine]) -> CheckResult {
    let ir_lines: Vec<&str> = ir.lines().collect();
    let mut failures = Vec::new();
    let mut search_from = 0;
    let mut last_match_line: Option<usize> = None;

    for dl in directives {
        match &dl.directive {
            Directive::Check { pattern } => match find_pattern(&ir_lines, pattern, search_from) {
                Some(found) => {
                    search_from = found + 1;
                    last_match_line = Some(found);
                }
                None => {
                    failures.push(CheckFailure::PatternNotFound {
                        source_line: dl.line_number,
                        pattern: pattern.clone(),
                    });
                }
            },
            Directive::CheckLabel { pattern } => {
                // Labels reset the search position
                match find_pattern(&ir_lines, pattern, 0) {
                    Some(found) => {
                        search_from = found + 1;
                        last_match_line = Some(found);
                    }
                    None => {
                        failures.push(CheckFailure::LabelNotFound {
                            source_line: dl.line_number,
                            pattern: pattern.clone(),
                        });
                    }
                }
            }
            Directive::CheckNot { pattern } => {
                if let Some(found) = find_pattern(&ir_lines, pattern, search_from) {
                    failures.push(CheckFailure::NegativePatternFound {
                        source_line: dl.line_number,
                        pattern: pattern.clone(),
                        found_at_line: found + 1,
                    });
                }
            }
            Directive::CheckNext { pattern } => {
                let expected = last_match_line.map_or(0, |l| l + 1);
                if expected < ir_lines.len() && ir_lines[expected].contains(pattern.as_str()) {
                    last_match_line = Some(expected);
                    search_from = expected + 1;
                } else {
                    // Try to find it elsewhere for better diagnostics
                    let actual = find_pattern(&ir_lines, pattern, search_from);
                    failures.push(if let Some(actual) = actual {
                        CheckFailure::NextNotAdjacent {
                            source_line: dl.line_number,
                            pattern: pattern.clone(),
                            expected_line: expected + 1,
                            actual_line: actual + 1,
                        }
                    } else {
                        CheckFailure::PatternNotFound {
                            source_line: dl.line_number,
                            pattern: pattern.clone(),
                        }
                    });
                }
            }
            _ => {}
        }
    }

    CheckResult {
        passed: failures.is_empty(),
        failures,
    }
}

/// Find a pattern in IR lines starting from `from` index.
fn find_pattern(lines: &[&str], pattern: &str, from: usize) -> Option<usize> {
    lines[from..]
        .iter()
        .position(|line| line.contains(pattern))
        .map(|pos| pos + from)
}

#[cfg(test)]
mod tests;
