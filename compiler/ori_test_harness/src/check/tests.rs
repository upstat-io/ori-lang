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

// CHECK-NOT bounded scoping tests

#[test]
fn exact_check_not_bounded_by_next_positive() {
    // CHECK-NOT should only check between current position and next CHECK match.
    // "bad_pattern" appears AFTER "line B", so CHECK-NOT between A and B should pass.
    let ir = "line A\nline B\nbad_pattern here\n";
    let directives = vec![
        dl(
            1,
            Directive::Check {
                pattern: "line A".into(),
            },
        ),
        dl(
            2,
            Directive::CheckNot {
                pattern: "bad_pattern".into(),
            },
        ),
        dl(
            3,
            Directive::Check {
                pattern: "line B".into(),
            },
        ),
    ];
    let result = run_checks(ir, &directives, CheckMode::Exact);
    assert!(
        result.passed,
        "CHECK-NOT should be bounded by next positive directive: {:?}",
        result.failures
    );
}

#[test]
fn exact_check_not_fails_within_bounded_region() {
    // "bad_pattern" appears between A and B — CHECK-NOT should catch it.
    let ir = "line A\nbad_pattern here\nline B\n";
    let directives = vec![
        dl(
            1,
            Directive::Check {
                pattern: "line A".into(),
            },
        ),
        dl(
            2,
            Directive::CheckNot {
                pattern: "bad_pattern".into(),
            },
        ),
        dl(
            3,
            Directive::Check {
                pattern: "line B".into(),
            },
        ),
    ];
    let result = run_checks(ir, &directives, CheckMode::Exact);
    assert!(
        !result.passed,
        "CHECK-NOT should fail when pattern is within bounded region"
    );
}

#[test]
fn exact_check_not_extends_to_eof_without_following_positive() {
    // No positive directive after CHECK-NOT — scans to EOF.
    let ir = "line A\nbad_pattern here\n";
    let directives = vec![
        dl(
            1,
            Directive::Check {
                pattern: "line A".into(),
            },
        ),
        dl(
            2,
            Directive::CheckNot {
                pattern: "bad_pattern".into(),
            },
        ),
    ];
    let result = run_checks(ir, &directives, CheckMode::Exact);
    assert!(
        !result.passed,
        "CHECK-NOT should extend to EOF when no following positive directive"
    );
}

#[test]
fn exact_check_not_ignores_patterns_in_later_functions() {
    // Simulates function-scoped behavior: CHECK-NOT between CHECK-LABEL
    // and next CHECK should not see patterns from a later function.
    let ir = "define @foo {\n  ret void\n}\ndefine @bar {\n  call @ori_rc_inc\n}\n";
    let directives = vec![
        dl(
            1,
            Directive::CheckLabel {
                pattern: "define @foo".into(),
            },
        ),
        dl(
            2,
            Directive::CheckNot {
                pattern: "ori_rc_inc".into(),
            },
        ),
        dl(
            3,
            Directive::Check {
                pattern: "ret void".into(),
            },
        ),
    ];
    let result = run_checks(ir, &directives, CheckMode::Exact);
    assert!(
        result.passed,
        "CHECK-NOT should not see patterns past the next positive directive: {:?}",
        result.failures
    );
}
