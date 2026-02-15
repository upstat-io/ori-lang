//! Specialized drop descriptor generation (Section 07.4).
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
pub fn compute_drop_info(
    ty: Idx,
    classifier: &dyn ArcClassification,
    pool: &Pool,
) -> Option<DropInfo> {
    if classifier.is_scalar(ty) {
        return None;
    }

    let kind = compute_drop_kind(ty, pool, classifier);

    Some(DropInfo { ty, kind })
}

/// Compute a drop descriptor for a closure environment.
///
/// Created from the capture types of a `PartialApply` instruction.
/// Each captured variable that needs RC gets an entry with its field
/// index in the env struct.
///
/// Returns [`DropKind::Trivial`] if no captures need RC.
pub fn compute_closure_env_drop(
    capture_types: &[Idx],
    classifier: &dyn ArcClassification,
) -> DropKind {
    let rc_captures: Vec<(u32, Idx)> = capture_types
        .iter()
        .enumerate()
        .filter_map(|(i, &ty)| {
            if classifier.needs_rc(ty) {
                #[expect(
                    clippy::cast_possible_truncation,
                    reason = "field indices are bounded by struct layout, never exceed u32"
                )]
                Some((i as u32, ty))
            } else {
                None
            }
        })
        .collect();

    if rc_captures.is_empty() {
        DropKind::Trivial
    } else {
        DropKind::ClosureEnv(rc_captures)
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
                if let ArcInstr::RcDec { var } = instr {
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

/// Compute the drop kind for a non-scalar type.
fn compute_drop_kind(ty: Idx, pool: &Pool, classifier: &dyn ArcClassification) -> DropKind {
    let (resolved_ty, resolved_tag) = resolve_type(ty, pool);

    match resolved_tag {
        // Collections

        // List: iterate elements if RC'd, otherwise just free buffer.
        Tag::List => {
            let elem = pool.list_elem(resolved_ty);
            if classifier.needs_rc(elem) {
                DropKind::Collection { element_type: elem }
            } else {
                DropKind::Trivial
            }
        }

        // Set: same structure as list.
        Tag::Set => {
            let elem = pool.set_elem(resolved_ty);
            if classifier.needs_rc(elem) {
                DropKind::Collection { element_type: elem }
            } else {
                DropKind::Trivial
            }
        }

        // Map: check keys and values independently.
        Tag::Map => {
            let key = pool.map_key(resolved_ty);
            let value = pool.map_value(resolved_ty);
            let dk = classifier.needs_rc(key);
            let dv = classifier.needs_rc(value);

            if dk || dv {
                DropKind::Map {
                    key_type: key,
                    value_type: value,
                    dec_keys: dk,
                    dec_values: dv,
                }
            } else {
                DropKind::Trivial
            }
        }

        // Fixed-layout compound types

        // Struct: Dec each RC'd field.
        Tag::Struct => compute_fields_drop(
            pool.struct_fields(resolved_ty)
                .into_iter()
                .map(|(_, ty)| ty),
            classifier,
        ),

        // Tuple: same as struct but indexed positionally.
        Tag::Tuple => compute_fields_drop(pool.tuple_elems(resolved_ty).into_iter(), classifier),

        // Enum: per-variant field drops.
        Tag::Enum => {
            let variants = pool.enum_variants(resolved_ty);
            compute_enum_drop(
                variants.into_iter().map(|(_, field_types)| field_types),
                classifier,
            )
        }

        // Tagged unions stored as special Pool types

        // option[T] → 2-variant enum: None (no fields), Some(T).
        Tag::Option => {
            let inner = pool.option_inner(resolved_ty);
            if classifier.needs_rc(inner) {
                DropKind::Enum(vec![
                    vec![],           // None — no RC'd fields
                    vec![(0, inner)], // Some — dec the inner value
                ])
            } else {
                DropKind::Trivial
            }
        }

        // result[T, E] → 2-variant enum: Ok(T), Err(E).
        Tag::Result => {
            let ok_ty = pool.result_ok(resolved_ty);
            let err_ty = pool.result_err(resolved_ty);
            let ok_rc = classifier.needs_rc(ok_ty);
            let err_rc = classifier.needs_rc(err_ty);

            if ok_rc || err_rc {
                DropKind::Enum(vec![
                    if ok_rc { vec![(0, ok_ty)] } else { vec![] },
                    if err_rc { vec![(0, err_ty)] } else { vec![] },
                ])
            } else {
                DropKind::Trivial
            }
        }

        // range[T] — ranges of RC types need bounds dropped.
        Tag::Range => {
            let elem = pool.range_elem(resolved_ty);
            if classifier.needs_rc(elem) {
                // range has start and end fields of the same type.
                DropKind::Fields(vec![(0, elem), (1, elem)])
            } else {
                DropKind::Trivial
            }
        }

        // Trivial fallback
        //
        // Named/Applied/Alias: resolve_type should have resolved
        //   these; unresolved named types get trivial drop.
        // Type variables, schemes, etc.: should be monomorphized
        //   before reaching codegen. Treat as trivial (just free).
        _ => DropKind::Trivial,
    }
}

/// Compute drop kind for a fixed-layout type with named/positional fields.
///
/// Returns `DropKind::Fields` if any field needs RC, `DropKind::Trivial`
/// if all fields are scalar.
fn compute_fields_drop(
    field_types: impl Iterator<Item = Idx>,
    classifier: &dyn ArcClassification,
) -> DropKind {
    let rc_fields: Vec<(u32, Idx)> = field_types
        .enumerate()
        .filter_map(|(i, field_ty)| {
            if classifier.needs_rc(field_ty) {
                #[expect(
                    clippy::cast_possible_truncation,
                    reason = "field indices are bounded by struct layout, never exceed u32"
                )]
                Some((i as u32, field_ty))
            } else {
                None
            }
        })
        .collect();

    if rc_fields.is_empty() {
        DropKind::Trivial
    } else {
        DropKind::Fields(rc_fields)
    }
}

/// Compute drop kind for an enum type with multiple variants.
///
/// Returns `DropKind::Enum` if any variant has RC'd fields,
/// `DropKind::Trivial` if no variant has RC'd fields.
fn compute_enum_drop(
    variants: impl Iterator<Item = Vec<Idx>>,
    classifier: &dyn ArcClassification,
) -> DropKind {
    let variant_drops: Vec<Vec<(u32, Idx)>> = variants
        .map(|field_types| {
            field_types
                .into_iter()
                .enumerate()
                .filter_map(|(i, field_ty)| {
                    if classifier.needs_rc(field_ty) {
                        #[expect(
                            clippy::cast_possible_truncation,
                            reason = "field indices are bounded by struct layout, never exceed u32"
                        )]
                        Some((i as u32, field_ty))
                    } else {
                        None
                    }
                })
                .collect()
        })
        .collect();

    if variant_drops.iter().all(Vec::is_empty) {
        DropKind::Trivial
    } else {
        DropKind::Enum(variant_drops)
    }
}

/// Resolve a type through Named/Alias indirection to its concrete tag.
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
