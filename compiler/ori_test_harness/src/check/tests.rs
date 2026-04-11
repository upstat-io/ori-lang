//! Tests for the `FileCheck` matching engine.

use super::*;
use crate::directive::{Directive, DirectiveLine};

fn dl(line: usize, dir: Directive) -> DirectiveLine {
    DirectiveLine {
        line_number: line,
        revision: None,
        directive: dir,
    }
}

// Matches mode tests

#[test]
fn matches_mode_finds_substring() {
    let ir = "define void @main() {\n  call void @ori_print()\n  ret void\n}\n";
    let directives = vec![dl(
        1,
        Directive::Check {
            pattern: "ori_print".into(),
        },
    )];
    let result = run_checks(ir, &directives, CheckMode::Matches);
    assert!(
        result.passed,
        "should find substring: {:?}",
        result.failures
    );
}

#[test]
fn matches_mode_reports_missing_pattern() {
    let ir = "define void @main() {\n  ret void\n}\n";
    let directives = vec![dl(
        1,
        Directive::Check {
            pattern: "ori_rc_inc".into(),
        },
    )];
    let result = run_checks(ir, &directives, CheckMode::Matches);
    assert!(!result.passed);
    assert_eq!(result.failures.len(), 1);
}

#[test]
fn check_not_fails_on_present_pattern() {
    let ir = "call void @ori_rc_inc(ptr %0)\nret void\n";
    let directives = vec![dl(
        1,
        Directive::CheckNot {
            pattern: "ori_rc_inc".into(),
        },
    )];
    let result = run_checks(ir, &directives, CheckMode::Matches);
    assert!(
        !result.passed,
        "CHECK-NOT should fail when pattern is found"
    );
}

#[test]
fn check_not_passes_when_absent() {
    let ir = "ret void\n";
    let directives = vec![dl(
        1,
        Directive::CheckNot {
            pattern: "ori_rc_inc".into(),
        },
    )];
    let result = run_checks(ir, &directives, CheckMode::Matches);
    assert!(result.passed);
}

// Exact mode tests

#[test]
fn exact_mode_respects_order() {
    let ir = "line A\nline B\nline C\n";
    let directives = vec![
        dl(
            1,
            Directive::Check {
                pattern: "line A".into(),
            },
        ),
        dl(
            2,
            Directive::Check {
                pattern: "line C".into(),
            },
        ),
    ];
    let result = run_checks(ir, &directives, CheckMode::Exact);
    assert!(result.passed);
}

#[test]
fn exact_mode_fails_on_wrong_order() {
    let ir = "line B\nline A\n";
    let directives = vec![
        dl(
            1,
            Directive::Check {
                pattern: "line A".into(),
            },
        ),
        dl(
            2,
            Directive::Check {
                pattern: "line B".into(),
            },
        ),
    ];
    // A is found at line 2, then B must be found AFTER line 2 — but B is at line 1.
    let result = run_checks(ir, &directives, CheckMode::Exact);
    assert!(!result.passed);
}

#[test]
fn check_label_resets_search_position() {
    let ir = "define @foo {\n  ret void\n}\ndefine @bar {\n  call @baz\n  ret void\n}\n";
    let directives = vec![
        dl(
            1,
            Directive::CheckLabel {
                pattern: "define @bar".into(),
            },
        ),
        dl(
            2,
            Directive::Check {
                pattern: "call @baz".into(),
            },
        ),
    ];
    let result = run_checks(ir, &directives, CheckMode::Exact);
    assert!(result.passed);
}

#[test]
fn check_next_requires_adjacent_line() {
    let ir = "line A\nline B\nline C\n";
    let directives = vec![
        dl(
            1,
            Directive::Check {
                pattern: "line A".into(),
            },
        ),
        dl(
            2,
            Directive::CheckNext {
                pattern: "line B".into(),
            },
        ),
    ];
    let result = run_checks(ir, &directives, CheckMode::Exact);
    assert!(result.passed);
}

#[test]
fn check_next_fails_when_not_adjacent() {
    let ir = "line A\nline X\nline B\n";
    let directives = vec![
        dl(
            1,
            Directive::Check {
                pattern: "line A".into(),
            },
        ),
        dl(
            2,
            Directive::CheckNext {
                pattern: "line B".into(),
            },
        ),
    ];
    let result = run_checks(ir, &directives, CheckMode::Exact);
    assert!(!result.passed);
}
