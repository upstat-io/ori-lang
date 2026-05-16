//! Minimal smoke test for `validate_partial_move` — full coverage lives
//! in the spec-test corpus at `compiler_repo/tests/spec/aims/`, which
//! exercises the validator end-to-end via `#compile_fail("E2043")`
//! directives.

use ori_ir::{ExprId, Name};
use rustc_hash::FxHashMap;

use super::validate_partial_move;
use crate::output::FunctionSig;
use crate::{Idx, Pool};

#[test]
fn partial_move_validator_handles_invalid_body_root_without_panicking() {
    let pool = Pool::new();
    let arena = ori_ir::ExprArena::new();
    let expr_types = FxHashMap::default();
    let sig = FunctionSig::simple(Name::EMPTY, vec![], Idx::UNIT);
    let mut errors = Vec::new();

    validate_partial_move(
        &pool,
        &arena,
        &expr_types,
        &sig,
        ExprId::INVALID,
        &mut errors,
    );

    assert!(
        errors.is_empty(),
        "ExprId::INVALID is the spec-canonical empty-body sentinel; \
         validator must short-circuit without emitting diagnostics"
    );
}
