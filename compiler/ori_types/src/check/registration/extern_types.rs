//! Extern type burden registration.
//!
//! Walks every `ExternBlock` in the module and registers per-extern-type
//! burden specs into the `TypeRegistry`. Two cases:
//!
//! - `#free(fn)` present → `UserBurdenSpec` with `user_drop = Some(FnSym)`,
//!   field/variant lists empty. Stored on the extern type's
//!   `TypeEntry.burden` slot.
//! - `#free` absent → no burden registered. The unified
//!   `ori_arc::lower::burden_lookup` helper falls back to
//!   `BurdenRef::Builtin(&ori_registry::burden::EMPTY_BURDEN_SPEC)` —
//!   semantically identical to builtin opaque types (`CPtr` / `JsValue`).
//!
//! Spec: Annex E §FFI — empty `BuiltinBurdenSpec` IS the soundness
//! boundary for caller-managed FFI lifetimes; passing such a type as
//! Owned without an annotation is rejected with E2042 downstream.
//!
//! # Status
//!
//! The current extern-block grammar (`extern_block = ... "{" { extern_item } "}"`)
//! permits only function-item declarations (`@name (...) -> T`); extern
//! type-declarations (`type Handle` inside an extern block) are
//! target-only per the future grammar extension carrying literals like
//! `extern "c" from "libsqlite" #free(sqlite3_close) { type DbHandle }`.
//! Until that AST surface ships, this helper iterates extern blocks but
//! produces no `TypeRegistry.burden` entries — the `free_fn` field on
//! `ExternBlock` is parsed and carried through the AST, ready for the
//! type-declaration consumer the grammar extension will add.

use core::num::NonZeroU32;

use ori_ir::{ExternBlock, Module};
use ori_registry::burden::FnSym;

use crate::registry::burden::UserBurdenSpec;
use crate::ModuleChecker;

/// Register burden specs for every type declared in every extern block.
///
/// Spec: Annex E §FFI. Pass 0b.5 — runs after `register_user_types` and
/// before signature collection, after struct/sum/newtype burden has been
/// computed but before any caller-site references can fire.
pub fn register_extern_burdens(checker: &mut ModuleChecker<'_>, module: &Module) {
    for extern_block in &module.extern_blocks {
        register_extern_block_burden(checker, extern_block);
    }
}

/// Process one extern block.
///
/// Today's extern-block AST carries function items only (`ExternItem`),
/// no type-item slot. The `free_fn` field is preserved on `ExternBlock`
/// and consulted here for the future extern-type-declaration code path:
///
/// ```text
/// extern "c" from "libsqlite" #free(sqlite3_close) {
///     type DbHandle  // target-only — see module-doc Status section
/// }
/// ```
///
/// When extern type declarations ship, this loop will compute one
/// `UserBurdenSpec` per declared type using `compute_extern_type_burden`
/// below and store it via `TypeRegistry.register_*` paths.
fn register_extern_block_burden(_checker: &mut ModuleChecker<'_>, _block: &ExternBlock) {
    // No extern-type AST surface today. `_block.free_fn` is preserved on
    // the AST and consumed by `compute_extern_type_burden` once the
    // type-declaration grammar extension lands.
}

/// Compute the `UserBurdenSpec` for an extern-declared opaque type.
///
/// Spec: Annex E §FFI — when `#free(fn)` is present, the spec carries
/// `user_drop = Some(fn)` and all field/variant lists empty (opaque types
/// have no Ori-visible fields). When absent, returns `None` — downstream
/// lookup falls back to `EMPTY_BURDEN_SPEC`.
///
/// Exposed for future extern-type-declaration registration.
//
// Staging helper for future extern-type AST consumer. In non-test builds
// this function has no caller until the type-declaration grammar lands.
#[must_use]
#[allow(
    dead_code,
    reason = "Staging helper for future extern-type AST consumer."
)]
pub fn compute_extern_type_burden(free_fn: Option<ori_ir::Name>) -> Option<UserBurdenSpec> {
    let free_fn = free_fn?;
    // Map Name's raw u32 into the FnSym's NonZeroU32 newtype. Name 0 is
    // EMPTY (reserved); free function symbols are user-defined identifiers
    // with non-zero raw values. Fallback to MIN is conservative but
    // unreachable in practice — the parser rejects empty / non-ident args
    // in `parse_extern_free_attr`.
    let raw = free_fn.raw();
    let nz = NonZeroU32::new(raw).unwrap_or(NonZeroU32::MIN);
    Some(UserBurdenSpec {
        self_owned_identity: false,
        owned_fields: vec![],
        borrowed_fields: vec![],
        variant_burdens: vec![],
        element_burden: None,
        drop_operation: None,
        user_drop: Some(FnSym::new(nz)),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use ori_ir::Name;

    fn unwrap_burden(burden: Option<UserBurdenSpec>) -> UserBurdenSpec {
        match burden {
            Some(b) => b,
            None => panic!("expected Some(UserBurdenSpec)"),
        }
    }

    fn unwrap_fn_sym(user_drop: Option<FnSym>) -> FnSym {
        match user_drop {
            Some(f) => f,
            None => panic!("expected user_drop = Some(FnSym)"),
        }
    }

    #[test]
    fn extern_burden_with_free_fn_carries_user_drop() {
        let raw = 42u32;
        let name = Name::from_raw(raw);
        let burden = unwrap_burden(compute_extern_type_burden(Some(name)));
        assert!(!burden.self_owned_identity);
        assert!(burden.owned_fields.is_empty());
        assert!(burden.borrowed_fields.is_empty());
        assert!(burden.variant_burdens.is_empty());
        assert!(burden.element_burden.is_none());
        assert!(burden.drop_operation.is_none());
        let fn_sym = unwrap_fn_sym(burden.user_drop);
        assert_eq!(fn_sym.get().get(), raw);
    }

    #[test]
    fn extern_burden_without_free_fn_returns_none() {
        let burden = compute_extern_type_burden(None);
        assert!(burden.is_none());
    }
}
