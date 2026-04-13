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
//!
//! # Known limitation: multiple-match flaw in `.matches` mode
//!
//! In `.matches` mode, each CHECK directive is tested independently
//! against the entire IR. Two identical `CHECK: ori_rc_inc` directives
//! will both match the **same** IR line — a test expecting N occurrences
//! of a pattern will pass with only 1 actual occurrence.
//!
//! **Mitigation**: For tests that care about occurrence count or ordering,
//! use `.exact` mode with `CHECK-LABEL` function anchoring. Alternatively,
//! use distinct substring patterns that include arguments (e.g.,
//! `CHECK: call void @ori_rc_inc(ptr %xs)` instead of bare
//! `CHECK: ori_rc_inc`).
//!
//! # CHECK-LABEL search behavior
//!
//! In `.exact` mode, `CHECK-LABEL` resets the search position to line 0
//! and scans forward for the label pattern. This means CHECK-LABEL
//! patterns should be specific enough to unambiguously identify the
//! target function — e.g., `CHECK-LABEL: define void @_ori_main` rather
//! than just `CHECK-LABEL: @main`. With function-scoped IR slicing
//! (where only the target function's IR is fed to the engine), CHECK-LABEL
//! is still useful for within-function structure (e.g., anchoring to a
//! specific basic block label) but is no longer the primary function
//! isolation mechanism.

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
/// the match to be on the immediately following line.
///
/// CHECK-NOT uses bounded scoping (standard LLVM `FileCheck` semantics):
/// a CHECK-NOT pattern is only rejected if it appears between the current
/// search position and the match position of the next positive directive
/// (CHECK, CHECK-LABEL, or CHECK-NEXT). If no subsequent positive directive
/// exists, CHECK-NOT extends to EOF. This prevents false positives from
/// patterns in unrelated functions or declarations.
fn run_exact_mode(ir: &str, directives: &[DirectiveLine]) -> CheckResult {
    let ir_lines: Vec<&str> = ir.lines().collect();
    let mut failures = Vec::new();
    let mut search_from = 0;
    let mut last_match_line: Option<usize> = None;

    for (i, dl) in directives.iter().enumerate() {
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
                // Labels reset the search position (search from line 0).
                // Patterns should be specific enough to unambiguously identify
                // the target — e.g., `define void @_ori_main` not just `@main`.
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
                // Standard LLVM FileCheck semantics: CHECK-NOT only checks
                // the region between the current search position and the
                // next positive directive's match (or EOF if none follows).
                let end = next_positive_bound(&ir_lines, &directives[i + 1..], search_from)
                    .unwrap_or(ir_lines.len());
                if let Some(found) = find_pattern_bounded(&ir_lines, pattern, search_from, end) {
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

/// Find the IR line where the next positive directive would match.
///
/// Used by CHECK-NOT to bound its search region. Scans forward through
/// remaining directives, returning the match position of the first positive
/// directive (CHECK, CHECK-LABEL, CHECK-NEXT) that matches. Returns `None`
/// if no subsequent positive directive matches (CHECK-NOT extends to EOF).
fn next_positive_bound(
    ir_lines: &[&str],
    remaining: &[DirectiveLine],
    from: usize,
) -> Option<usize> {
    for dl in remaining {
        match &dl.directive {
            Directive::Check { pattern } | Directive::CheckNext { pattern } => {
                if let Some(pos) = find_pattern(ir_lines, pattern, from) {
                    return Some(pos);
                }
            }
            Directive::CheckLabel { pattern } => {
                // Labels search from 0 (they reset position)
                if let Some(pos) = find_pattern(ir_lines, pattern, 0) {
                    return Some(pos);
                }
            }
            _ => {}
        }
    }
    None
}

/// Find a pattern in IR lines starting from `from` index.
fn find_pattern(lines: &[&str], pattern: &str, from: usize) -> Option<usize> {
    lines[from..]
        .iter()
        .position(|line| line.contains(pattern))
        .map(|pos| pos + from)
}

/// Find a pattern in IR lines within a bounded range `[from, end)`.
fn find_pattern_bounded(lines: &[&str], pattern: &str, from: usize, end: usize) -> Option<usize> {
    let end = end.min(lines.len());
    lines[from..end]
        .iter()
        .position(|line| line.contains(pattern))
        .map(|pos| pos + from)
}

#[cfg(test)]
mod tests;
