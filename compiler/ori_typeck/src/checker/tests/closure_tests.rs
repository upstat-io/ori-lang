//! Tests for closure self-capture detection.

use super::check_source;

#[test]
fn test_closure_self_capture_direct() {
    let (_, typed) = check_source(
        r"
            @test () -> int = run(
                let f = () -> f,
                0
            )
        ",
    );

    assert!(typed.has_errors());
    assert!(typed
        .errors
        .iter()
        .any(|e| e.message.contains("closure cannot capture itself")
            && e.code == ori_diagnostic::ErrorCode::E2007));
}

#[test]
fn test_closure_self_capture_call() {
    let (_, typed) = check_source(
        r"
            @test () -> int = run(
                let f = (x: int) -> f(x: x + 1),
                0
            )
        ",
    );

    assert!(typed.has_errors());
    assert!(typed
        .errors
        .iter()
        .any(|e| e.message.contains("closure cannot capture itself")));
}

#[test]
fn test_no_self_capture_uses_outer_binding() {
    let (_, typed) = check_source(
        r"
            @test () -> int = run(
                let f = 42,
                let g = () -> f,
                g()
            )
        ",
    );

    assert!(!typed
        .errors
        .iter()
        .any(|e| e.code == ori_diagnostic::ErrorCode::E2007));
}

#[test]
fn test_no_self_capture_non_lambda() {
    let (_, typed) = check_source(
        r"
            @test () -> int = run(
                let x = 1 + 2,
                x
            )
        ",
    );

    assert!(!typed.has_errors());
}

#[test]
fn test_closure_self_capture_in_run() {
    let (_, typed) = check_source(
        r"
            @test () -> int = run(
                let f = () -> f,
                0
            )
        ",
    );

    assert!(typed.has_errors());
    assert!(typed
        .errors
        .iter()
        .any(|e| e.message.contains("closure cannot capture itself")));
}

#[test]
fn test_closure_self_capture_nested_expression() {
    let (_, typed) = check_source(
        r"
            @test () -> int = run(
                let f = () -> if true then f else f,
                0
            )
        ",
    );

    assert!(typed.has_errors());
    assert!(typed
        .errors
        .iter()
        .any(|e| e.message.contains("closure cannot capture itself")));
}

#[test]
fn test_valid_mutual_recursion_via_outer_scope() {
    let (_, typed) = check_source(
        r"
            @f (x: int) -> int = x
            @test () -> int = run(
                let g = (x: int) -> @f(x: x),
                g(x: 1)
            )
        ",
    );

    assert!(!typed
        .errors
        .iter()
        .any(|e| e.code == ori_diagnostic::ErrorCode::E2007));
}
