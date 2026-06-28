//! Smoke tests for `validate_partial_move` (E2043) — full coverage lives in the
//! spec-test corpus at `tests/spec/aims/`, which exercises the
//! validator end-to-end via `#compile_fail("E2043")` directives.

use ori_ir::ExprId;
use rustc_hash::FxHashMap;

use super::validate_partial_move;
use crate::Pool;

#[test]
fn partial_move_validator_handles_invalid_body_root_without_panicking() {
    let pool = Pool::new();
    let arena = ori_ir::ExprArena::new();
    let expr_types = FxHashMap::default();
    let mut errors = Vec::new();

    validate_partial_move(&pool, &arena, &expr_types, ExprId::INVALID, &mut errors);

    assert!(
        errors.is_empty(),
        "ExprId::INVALID is the spec-canonical empty-body sentinel; \
         validator must short-circuit without emitting diagnostics"
    );
}
