//! Specialized drop descriptor generation.
//!
//! When a reference-counted value's refcount reaches zero, its memory must
//! be cleaned up: all RC'd child fields must be decremented before the
//! allocation is freed. This module computes per-type **drop descriptors**
//! that tell the codegen layer exactly what cleanup is needed.
//!
//! # Design
//!
//! Drop descriptors are **declarative** — they describe WHAT to clean up,
//! not HOW. The codegen layer (`ori_llvm`) uses these descriptors to
//! generate actual drop functions (LLVM IR functions with the cleanup
//! logic). This keeps `ori_arc` backend-independent while centralizing
//! the "which fields need RC" analysis.
//!
//! Two categories of reference-counted types:
//!
//! - **Self-RC**: types behind their own refcount (`str`, `[T]`, `{K:V}`,
//!   closures). Codegen emits `ori_rc_dec(ptr, drop_fn)`.
//! - **Transitive-RC**: stack types containing RC children (`option[str]`,
//!   `(int, str)`, custom structs). Codegen emits inline destructure +
//!   Dec children. The drop descriptor is the same — the codegen layer
//!   decides the emission strategy based on `TypeInfo`.
//!
//! # Reference Compilers
//!
//! - **Lean 4** `src/Lean/Compiler/IR/RC.lean` — type-specific cleanup
//!   in `addDec`, object header carries layout metadata
//! - **Roc** `crates/compiler/mono/src/code_gen_help/refcount.rs` —
//!   per-layout refcount helpers, specialized drop per type

use rustc_hash::FxHashSet;

use ori_types::{Idx, Pool, Tag};

use crate::ir::{ArcFunction, ArcInstr};
use crate::ArcClassification;

mod burden_bridge;

// Drop descriptor types

/// Describes the cleanup needed when a reference-counted value's
/// refcount reaches zero.
///
/// Each variant corresponds to a different kind of type and specifies
/// exactly which child fields need `RcDec` before freeing the memory.
/// The codegen layer uses `field_type` entries to look up each child's
/// own drop function, enabling recursive drop.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "cache", derive(serde::Serialize, serde::Deserialize))]
pub enum DropKind {
    /// No RC'd children — just free the allocation.
    ///
    /// Examples: `str` (bytes aren't RC'd), `[int]` (elements are
    /// scalar), `chan<int>` (runtime-managed), bare function pointers.
    Trivial,

    /// Fixed-layout type (struct, tuple): Dec specific RC'd fields.
    ///
    /// Each entry is `(field_index, field_type)` for fields that need
    /// RC. The codegen layer uses `field_index` for `struct_gep` and
    /// `field_type` to look up the field's own drop function.
    Fields(Vec<(u32, Idx)>),

    /// Enum type: switch on tag, then Dec variant-specific RC'd fields.
    ///
    /// Outer `Vec` indexed by variant ordinal. Inner `Vec` contains
    /// `(field_index, field_type)` for fields in that variant needing
    /// RC. An empty inner `Vec` means the variant has no RC'd fields.
    ///
    /// Also used for `option[T]` (2 variants: None, Some) and
    /// `result[T, E]` (2 variants: Ok, Err).
    Enum(Vec<Vec<(u32, Idx)>>),

    /// Variable-length collection (`[T]`, `set[T]`): iterate elements,
    /// Dec each.
    ///
    /// Only created when elements are RC'd. If elements are scalar,
    /// [`Trivial`](DropKind::Trivial) is used instead.
    Collection {
        /// The element type (each element needs `RcDec`).
        element_type: Idx,
    },

    /// Map type (`{K: V}`): iterate entries, Dec keys and/or values.
    ///
    /// At least one of `dec_keys`/`dec_values` is true (otherwise
    /// [`Trivial`](DropKind::Trivial) would be used).
    Map {
        key_type: Idx,
        value_type: Idx,
        /// Whether keys need `RcDec`.
        dec_keys: bool,
        /// Whether values need `RcDec`.
        dec_values: bool,
    },

    /// Closure environment: Dec specific captured variables.
    ///
    /// Structurally identical to [`Fields`](DropKind::Fields) but
    /// semantically distinct for naming (`_ori_drop$__lambda_N_env`).
    ClosureEnv(Vec<(u32, Idx)>),
}

/// Complete drop information for a type.
///
/// Returned by [`compute_drop_info`] for types that need RC cleanup.
/// Scalar types (which don't need RC) return `None` from
/// `compute_drop_info`.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "cache", derive(serde::Serialize, serde::Deserialize))]
pub struct DropInfo {
    /// The type this drop info describes.
    pub ty: Idx,
    /// What cleanup operations are needed.
    pub kind: DropKind,
}

// Core API

/// Compute the drop descriptor for a type.
///
/// Returns `None` for scalar types (no RC, no drop needed).
/// Returns `Some(DropInfo)` for reference-counted types.
///
/// The drop descriptor tells the codegen layer what cleanup is needed
/// when this type's refcount reaches zero:
/// - Which child fields need `RcDec`
/// - Whether to iterate elements (for collections)
/// - Whether to switch on enum tag
///
/// Transitional thin-wrapper preserved during the burden-tracking
/// migration. Synthesises a [`burden_bridge::SynthesizedBurden`] from the
/// pool walk + classifier and forwards through
/// [`burden_bridge::burden_to_drop_info`]. Consumers should migrate to
/// direct `BurdenRegistry::lookup_burden` calls at their own pace.
/// Full deprecation lands at §06 (Phase 7 mechanical lowering) for the
/// `ori_arc::lower::burden_lower` emission boundary and §09 (post-
/// convergence partial retirement) for the
/// `ori_llvm::codegen::arc_emitter::element_fn_gen` cross-crate caller —
/// the only cross-crate production consumer (verified zero `ori_eval`
/// callers in §02.3.B audit).
pub fn compute_drop_info(
    ty: Idx,
    classifier: &dyn ArcClassification,
    pool: &Pool,
) -> Option<DropInfo> {
    if classifier.is_scalar(ty) {
        return None;
    }

    // Iterators are non-trivial (they need `ori_iter_drop`)
    // but they do NOT use per-type drop functions. The ARC emitter
    // dispatches iterator drops inline via `RcStrategy::Iterator` (for
    // top-level RcDec on iterator variables) and the `Tag::Iterator`
    // arm of `dec_value_rc_inner` (for iterator fields inside compound
    // types). Both paths emit `ori_iter_drop(ptr)` directly. Returning
    // `None` here prevents `collect_drop_infos` from asking `drop_gen`
    // to generate a spurious `_ori_drop$Iterator<T>` function, which
    // would call `ori_rc_free` on a Box-allocated pointer and corrupt
    // memory.
    let (_, tag) = resolve_type(ty, pool);
    if matches!(tag, Tag::Iterator | Tag::DoubleEndedIterator) {
        return None;
    }

    // BurdenSpec lift: synthesise a `UserBurdenSpec` (plus a
    // discriminator for shape-disambiguation that `UserBurdenSpec`
    // itself does not carry) and lift it to `DropInfo`. When the type
    // is genuinely trivial after walking (all-scalar fields, all-scalar
    // variants), `synthesize_burden_from_pool` returns `None`; codegen
    // matches the legacy `compute_drop_kind` → `DropKind::Trivial`
    // semantics by emitting `DropInfo { ty, kind: Trivial }`.
    match burden_bridge::synthesize_burden_from_pool(ty, classifier, pool) {
        Some(synthesized) => {
            burden_bridge::burden_to_drop_info(ty, &synthesized).or(Some(DropInfo {
                ty,
                kind: DropKind::Trivial,
            }))
        }
        None => Some(DropInfo {
            ty,
            kind: DropKind::Trivial,
        }),
    }
}

/// Compute a drop descriptor for a closure environment.
///
/// Created from the capture types of a `PartialApply` instruction.
/// Each captured variable that needs RC gets an entry with its field
/// index in the env struct.
///
/// Returns [`DropKind::Trivial`] if no captures need RC.
///
/// Transitional thin-wrapper preserved during the burden-tracking
/// migration. Synthesises a [`burden_bridge::SynthesizedBurden`] for the
/// closure environment and forwards through
/// [`burden_bridge::burden_to_drop_info`]. Closure environments are
/// constructed by `PartialApply` ARC-IR lowering and never interned in
/// the type pool, so the closure-specific synthesizer takes the capture
/// slice directly.
pub fn compute_closure_env_drop(
    capture_types: &[Idx],
    classifier: &dyn ArcClassification,
) -> DropKind {
    match burden_bridge::synthesize_closure_env_burden(capture_types, classifier) {
        Some(synthesized) => burden_bridge::burden_to_drop_info(
            // The pool index is irrelevant for closure env drops — codegen
            // names the drop function by the lambda's synthesized identity
            // (`_ori_drop$__lambda_N_env`), not by `DropInfo.ty`. Use a
            // sentinel `Idx::ERROR` for the shell so a stray consumer that
            // forgets the closure-env special case surfaces loudly.
            Idx::ERROR,
            &synthesized,
        )
        .map_or(DropKind::Trivial, |info| info.kind),
        None => DropKind::Trivial,
    }
}

/// Collect drop infos for all types that appear in `RcDec` instructions
/// across the given functions.
///
/// Returns a deduplicated list of [`DropInfo`] descriptors. The codegen
/// layer uses these to generate specialized drop functions.
///
/// **Note:** This collects types from `RcDec` instructions only. For
/// nested types (e.g., a struct field's type), the codegen layer should
/// call [`compute_drop_info`] lazily when generating each drop function.
pub fn collect_drop_infos(
    functions: &[ArcFunction],
    classifier: &dyn ArcClassification,
    pool: &Pool,
) -> Vec<DropInfo> {
    let mut seen = FxHashSet::default();
    let mut infos = Vec::new();

    for func in functions {
        for block in &func.blocks {
            for instr in &block.body {
                if let ArcInstr::RcDec { var, .. } = instr {
                    let ty = func.var_type(*var);
                    if classifier.needs_rc(ty) && seen.insert(ty) {
                        if let Some(info) = compute_drop_info(ty, classifier, pool) {
                            infos.push(info);
                        }
                    }
                }
            }
        }
    }

    infos
}

// Internal helpers

/// Resolve a type through Named/Alias indirection to its concrete tag.
///
/// Used by the iterator early-exit guard in [`compute_drop_info`]. The
/// full pool walk lives in
/// [`burden_bridge::synthesize_burden_from_pool`]; this helper is
/// retained because the iterator guard fires BEFORE the bridge synthesises
/// any burden (iterator drops are emitted inline by the ARC emitter, not
/// per-type drop functions).
fn resolve_type(ty: Idx, pool: &Pool) -> (Idx, Tag) {
    let tag = pool.tag(ty);
    match tag {
        Tag::Named | Tag::Applied | Tag::Alias => match pool.resolve(ty) {
            Some(resolved) => resolve_type(resolved, pool),
            None => (ty, tag),
        },
        _ => (ty, tag),
    }
}

// Tests

#[cfg(test)]
#[expect(
    clippy::unwrap_used,
    reason = "tests use unwrap for concise assertions"
)]
mod tests;
