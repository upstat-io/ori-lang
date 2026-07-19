//! FFI exclusion contract — empty vs annotated `BurdenSpec` for opaque types.
//!
//! Spec: Annex E §FFI Q12. The contract is bidirectional:
//!
//! - Unannotated FFI / opaque types (`CPtr`, `JsValue`, `JsPromise<T>`, and
//!   `extern "c" from "lib" { ... }` types WITHOUT `#free` annotation) get
//!   an EMPTY `BuiltinBurdenSpec` (`EMPTY_BURDEN_SPEC` below): no fields, no
//!   variants, `self_owned_identity = false`, `drop_operation = None`,
//!   `user_drop = None`. Sound ONLY for caller-managed lifetime — Ori does
//!   NOT decrement; the FFI consumer owns cleanup.
//!
//! - Annotated FFI types (`extern "c" from "lib" #free(fn) { ... }`) get an
//!   explicit `UserBurdenSpec` (owned-vector storage in `ori_types`) with
//!   `user_drop: Some(FnSym)` pointing at the user-supplied free function.
//!
//! Spec: Annex E §AIMS RL-31 Sufficient-Noalias Rule clause 8 explicitly
//! excludes empty-`BuiltinBurdenSpec` FFI parameters from the RL-31 noalias
//! proof — empty `BurdenSpec` ≠ empty reachable memory.
//
// Spec: Annex E §FFI — empty `BuiltinBurdenSpec` IS the soundness boundary;
// `EMPTY_BURDEN_SPEC` is the canonical fallback consumed by the unified
// burden-lookup helper in `ori_arc::lower::burden_lookup` when an extern
// type carries no `#free` annotation.

use super::BuiltinBurdenSpec;

/// Static all-empty `BuiltinBurdenSpec` for unannotated opaque FFI types.
///
/// Spec: Annex E §FFI. Returned by `lookup_burden` when
/// an extern type was registered WITHOUT a `#free(fn)` annotation — the
/// type's lifetime is caller-managed; Ori emits no drops; passing such a
/// value to an Owned-expecting position is rejected with E2042.
///
/// Semantically identical to the builtin entries for `CPtr` / `JsValue` /
/// `JsPromise<T>` should those types acquire `TypeTag` representation;
/// today those names resolve through `Tag::Named` rather than the registry
/// `BURDEN_TABLE`, so the unannotated-extern fallback path is the single
/// surface they share.
pub static EMPTY_BURDEN_SPEC: BuiltinBurdenSpec = BuiltinBurdenSpec::EMPTY;
