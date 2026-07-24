//! Smoke tests for `validate_consumed_binding` (E2054). Full end-to-end
//! coverage lives in the spec-test corpus at
//! `tests/spec/aims/drop_early/`, which exercises the validator
//! via `#compile_fail("E2054")` directives on real `drop_early` programs.

use ori_ir::{ExprId, Name};

use super::validate_consumed_binding;

#[test]
fn consumed_binding_validator_handles_invalid_body_root_without_panicking() {
    let arena = ori_ir::ExprArena::new();
    let mut errors = Vec::new();

    validate_consumed_binding(&arena, Name::EMPTY, ExprId::INVALID, &mut errors);

    assert!(
        errors.is_empty(),
        "ExprId::INVALID is the spec-canonical empty-body sentinel; \
         validator must short-circuit without emitting diagnostics"
    );
}

#[test]
fn consumed_binding_validator_empty_arena_no_drop_early_is_silent() {
    // A body with no `drop_early` call surface never consumes anything; the
    // walk threads an empty consumed set and emits nothing.
    let mut arena = ori_ir::ExprArena::new();
    let body = arena.alloc_expr(ori_ir::Expr {
        kind: ori_ir::ExprKind::Int(0),
        span: ori_ir::Span::DUMMY,
    });
    let mut errors = Vec::new();

    validate_consumed_binding(&arena, Name::EMPTY, body, &mut errors);

    assert!(
        errors.is_empty(),
        "a literal body consumes nothing — no E2054 expected"
    );
}
