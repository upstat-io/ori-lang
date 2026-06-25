//! Closure capture composition — `UserBurdenSpec` population at
//! closure-type-registration time.
//!
//! Spec: Annex E §AIMS — closure environments are heap-allocated values whose
//! burden is composed at the lambda-expression type-check site. The composed
//! `UserBurdenSpec` carries:
//!
//! - `self_heap_alloc: true` — closure env is heap-allocated.
//! - `owned_fields[i]` — one entry per captured-by-value capture (`Idx` = the
//!   captured binding's resolved type Idx).
//! - `borrowed_fields[i]` — one entry per captured-by-reference capture (per
//!   `Tag::Borrowed`-target lifetime tie-back to parent variable; the closure
//!   env-header layout carries the refcount + `drop_fn` slot ahead of captures).
//! - `element_burden: None` — closures are not collections.
//! - `variant_burdens: []` — closures are not sums.
//! - `compiled_drop: Some(env_drop_fn_sym)` — minted at registration, body
//!   materialized later via existing `DropKind::ClosureEnv` arm at
//!   `compiler_repo/compiler/ori_llvm/src/codegen/arc_emitter/drop_gen.rs`.
//! - `user_drop: None` — closures cannot have user `@drop` (only types do per
//!   `drop-trait-proposal.md §Auto-derive`).
//!
//! Env-header layout (NO new ABI — already shipped in `rc_ops.rs`):
//!
//! ```text
//! [env_ptr - 8] : refcount (i64, RT-2 header convention)
//! [env_ptr + 0] : drop_fn (ptr, 8 bytes — loaded by emit_rc_dec_closure)
//! [env_ptr + 8] : captured_field_0
//! [env_ptr + 16]: captured_field_1
//! ...
//! ```
//!
//! `field_path` indices in `UserOwnedField` / `UserBorrowedField` start at 0
//! (logical capture position). The +8-byte physical offset from the env-header
//! `drop_fn` slot is handled by codegen at `gep env_ptr, 8 + accumulated_offsets`,
//! NOT by the burden spec (which carries logical capture indices for the
//! `DropKind::ClosureEnv(fields)` arm — see `drop_gen.rs`).

use ori_registry::burden::FnSym;

use super::scc::mint_compiled_drop_fn_sym;
use crate::registry::burden::{
    UserBorrowedField, UserBurdenSpec, UserOwnedField, UserVariantBurden,
};
use crate::Idx;

/// A single capture descriptor used to compose a closure's `UserBurdenSpec`.
///
/// `field_index` is the logical capture position (0-based) — corresponds to the
/// caller's argument order at the `PartialApply` site. `field_type` is the
/// resolved `Idx` of the captured binding's type.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct ClosureCapture {
    pub field_index: u32,
    pub field_type: Idx,
}

/// Compose a `UserBurdenSpec` for a closure type given its captured bindings.
///
/// Inputs:
/// - `closure_idx` — the resolved `Idx` of the synthesized closure type. The
///   minted `compiled_drop` `FnSym` keys on this `Idx` via
///   `mint_compiled_drop_fn_sym` (per `scc/mod.rs` mangling: `raw + 1`).
/// - `owned_captures` — captured-by-value captures. One `UserOwnedField` per
///   entry; `field_path: vec![capture.field_index]`.
/// - `borrowed_captures` — captured-by-reference captures. One
///   `UserBorrowedField` per entry; same `field_path` convention.
///
/// Output: `UserBurdenSpec` populated as:
/// - `self_heap_alloc: true`
/// - `owned_fields` / `borrowed_fields` mapped from inputs
/// - `element_burden: None`, `variant_burdens: []`
/// - `compiled_drop: Some(mint_compiled_drop_fn_sym(closure_idx))`
/// - `user_drop: None`
///
/// The composed spec is then registered via
/// `TypeRegistry::register_user_burden(closure_idx, spec)` at the lambda's
/// type-check site (`ori_types::infer::expr::infer_lambda` per
/// `compiler_repo/compiler/ori_types/src/infer/expr/blocks.rs`).
#[must_use]
pub fn compose_closure_burden_spec(
    closure_idx: Idx,
    owned_captures: &[ClosureCapture],
    borrowed_captures: &[ClosureCapture],
) -> UserBurdenSpec {
    UserBurdenSpec {
        self_heap_alloc: true,
        owned_fields: owned_captures
            .iter()
            .map(|c| UserOwnedField {
                field_path: vec![c.field_index],
                field_type: c.field_type,
            })
            .collect(),
        borrowed_fields: borrowed_captures
            .iter()
            .map(|c| UserBorrowedField {
                field_path: vec![c.field_index],
                field_type: c.field_type,
            })
            .collect(),
        variant_burdens: Vec::<UserVariantBurden>::new(),
        element_burden: None,
        compiled_drop: Some(mint_closure_drop_fn_sym(closure_idx)),
        user_drop: None,
    }
}

/// Mint a deterministic `FnSym` for a closure's compiled drop function.
///
/// Reuses `scc::mint_compiled_drop_fn_sym` so closure drop functions share the
/// `_ori_drop$<idx_raw>` mangling key with recursive-type drop functions per
/// `drop_gen.rs`. The per-`Idx` distinctness contract (`raw + 1`, saturating)
/// holds identically — two closures with distinct pool `Idx` values get distinct
/// `FnSym` values even when their capture shapes are structurally identical.
#[must_use]
pub fn mint_closure_drop_fn_sym(closure_idx: Idx) -> FnSym {
    mint_compiled_drop_fn_sym(closure_idx)
}

#[cfg(test)]
mod tests;
