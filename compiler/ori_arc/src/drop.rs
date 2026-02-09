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
use crate::{ArcClassification, ArcClassifier};

// ── Drop descriptor types ──────────────────────────────────────────

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

// ── Core API ───────────────────────────────────────────────────────

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
pub fn compute_drop_info(ty: Idx, classifier: &ArcClassifier) -> Option<DropInfo> {
    if classifier.is_scalar(ty) {
        return None;
    }

    let pool = classifier.pool();
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
pub fn compute_closure_env_drop(capture_types: &[Idx], classifier: &ArcClassifier) -> DropKind {
    let rc_captures: Vec<(u32, Idx)> = capture_types
        .iter()
        .enumerate()
        .filter_map(|(i, &ty)| {
            if classifier.needs_rc(ty) {
                #[allow(clippy::cast_possible_truncation)] // field index < u32::MAX
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
pub fn collect_drop_infos(functions: &[ArcFunction], classifier: &ArcClassifier) -> Vec<DropInfo> {
    let mut seen = FxHashSet::default();
    let mut infos = Vec::new();

    for func in functions {
        for block in &func.blocks {
            for instr in &block.body {
                if let ArcInstr::RcDec { var } = instr {
                    let ty = func.var_type(*var);
                    if classifier.needs_rc(ty) && seen.insert(ty) {
                        if let Some(info) = compute_drop_info(ty, classifier) {
                            infos.push(info);
                        }
                    }
                }
            }
        }
    }

    infos
}

// ── Internal helpers ───────────────────────────────────────────────

/// Compute the drop kind for a non-scalar type.
fn compute_drop_kind(ty: Idx, pool: &Pool, classifier: &ArcClassifier) -> DropKind {
    let (resolved_ty, resolved_tag) = resolve_type(ty, pool);

    match resolved_tag {
        // ── Collections ────────────────────────────────────────

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

        // ── Fixed-layout compound types ────────────────────────

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

        // ── Tagged unions stored as special Pool types ─────────

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

        // ── Trivial fallback ──────────────────────────────────────
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
    classifier: &ArcClassifier,
) -> DropKind {
    let rc_fields: Vec<(u32, Idx)> = field_types
        .enumerate()
        .filter_map(|(i, field_ty)| {
            if classifier.needs_rc(field_ty) {
                #[allow(clippy::cast_possible_truncation)] // field index < u32::MAX
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
    classifier: &ArcClassifier,
) -> DropKind {
    let variant_drops: Vec<Vec<(u32, Idx)>> = variants
        .map(|field_types| {
            field_types
                .into_iter()
                .enumerate()
                .filter_map(|(i, field_ty)| {
                    if classifier.needs_rc(field_ty) {
                        #[allow(clippy::cast_possible_truncation)] // field index < u32::MAX
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

// ── Tests ──────────────────────────────────────────────────────────

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use ori_ir::Name;
    use ori_types::{EnumVariant, Idx, Pool};
    use pretty_assertions::assert_eq;

    use crate::ir::{
        ArcBlock, ArcBlockId, ArcFunction, ArcInstr, ArcParam, ArcTerminator, ArcVarId,
    };
    use crate::{ArcClassifier, Ownership};

    use super::*;

    // ── Helper ─────────────────────────────────────────────────

    fn cls(pool: &Pool) -> ArcClassifier<'_> {
        ArcClassifier::new(pool)
    }

    // ── Scalar types → None ────────────────────────────────────

    #[test]
    fn scalar_returns_none() {
        let pool = Pool::new();
        let c = cls(&pool);

        assert!(compute_drop_info(Idx::INT, &c).is_none());
        assert!(compute_drop_info(Idx::FLOAT, &c).is_none());
        assert!(compute_drop_info(Idx::BOOL, &c).is_none());
        assert!(compute_drop_info(Idx::CHAR, &c).is_none());
        assert!(compute_drop_info(Idx::UNIT, &c).is_none());
    }

    #[test]
    fn option_of_scalar_returns_none() {
        let mut pool = Pool::new();
        let opt_int = pool.option(Idx::INT);
        let c = cls(&pool);

        assert!(compute_drop_info(opt_int, &c).is_none());
    }

    #[test]
    fn tuple_of_scalars_returns_none() {
        let mut pool = Pool::new();
        let tup = pool.tuple(&[Idx::INT, Idx::FLOAT, Idx::BOOL]);
        let c = cls(&pool);

        assert!(compute_drop_info(tup, &c).is_none());
    }

    #[test]
    fn struct_all_scalar_returns_none() {
        let mut pool = Pool::new();
        let s = pool.struct_type(
            Name::from_raw(10),
            &[
                (Name::from_raw(11), Idx::INT),
                (Name::from_raw(12), Idx::FLOAT),
            ],
        );
        let c = cls(&pool);

        assert!(compute_drop_info(s, &c).is_none());
    }

    #[test]
    fn enum_all_unit_variants_returns_none() {
        let mut pool = Pool::new();
        let e = pool.enum_type(
            Name::from_raw(20),
            &[
                EnumVariant {
                    name: Name::from_raw(21),
                    field_types: vec![],
                },
                EnumVariant {
                    name: Name::from_raw(22),
                    field_types: vec![],
                },
            ],
        );
        let c = cls(&pool);

        assert!(compute_drop_info(e, &c).is_none());
    }

    // ── str → Trivial ──────────────────────────────────────────

    #[test]
    fn str_is_trivial() {
        let pool = Pool::new();
        let c = cls(&pool);

        let info = compute_drop_info(Idx::STR, &c).unwrap();
        assert_eq!(info.ty, Idx::STR);
        assert_eq!(info.kind, DropKind::Trivial);
    }

    // ── List ───────────────────────────────────────────────────

    #[test]
    fn list_of_scalar_is_trivial() {
        let mut pool = Pool::new();
        let list = pool.list(Idx::INT);
        let c = cls(&pool);

        let info = compute_drop_info(list, &c).unwrap();
        assert_eq!(info.kind, DropKind::Trivial);
    }

    #[test]
    fn list_of_str_is_collection() {
        let mut pool = Pool::new();
        let list = pool.list(Idx::STR);
        let c = cls(&pool);

        let info = compute_drop_info(list, &c).unwrap();
        assert_eq!(
            info.kind,
            DropKind::Collection {
                element_type: Idx::STR
            }
        );
    }

    #[test]
    fn list_of_list_is_collection() {
        let mut pool = Pool::new();
        let inner = pool.list(Idx::INT);
        let outer = pool.list(inner);
        let c = cls(&pool);

        let info = compute_drop_info(outer, &c).unwrap();
        assert_eq!(
            info.kind,
            DropKind::Collection {
                element_type: inner
            }
        );
    }

    // ── Set ────────────────────────────────────────────────────

    #[test]
    fn set_of_scalar_is_trivial() {
        let mut pool = Pool::new();
        let set = pool.set(Idx::INT);
        let c = cls(&pool);

        let info = compute_drop_info(set, &c).unwrap();
        assert_eq!(info.kind, DropKind::Trivial);
    }

    #[test]
    fn set_of_str_is_collection() {
        let mut pool = Pool::new();
        let set = pool.set(Idx::STR);
        let c = cls(&pool);

        let info = compute_drop_info(set, &c).unwrap();
        assert_eq!(
            info.kind,
            DropKind::Collection {
                element_type: Idx::STR
            }
        );
    }

    // ── Map ────────────────────────────────────────────────────

    #[test]
    fn map_scalar_keys_and_values_is_trivial() {
        let mut pool = Pool::new();
        let map = pool.map(Idx::INT, Idx::FLOAT);
        let c = cls(&pool);

        let info = compute_drop_info(map, &c).unwrap();
        assert_eq!(info.kind, DropKind::Trivial);
    }

    #[test]
    fn map_str_keys_scalar_values() {
        let mut pool = Pool::new();
        let map = pool.map(Idx::STR, Idx::INT);
        let c = cls(&pool);

        let info = compute_drop_info(map, &c).unwrap();
        assert_eq!(
            info.kind,
            DropKind::Map {
                key_type: Idx::STR,
                value_type: Idx::INT,
                dec_keys: true,
                dec_values: false,
            }
        );
    }

    #[test]
    fn map_scalar_keys_str_values() {
        let mut pool = Pool::new();
        let map = pool.map(Idx::INT, Idx::STR);
        let c = cls(&pool);

        let info = compute_drop_info(map, &c).unwrap();
        assert_eq!(
            info.kind,
            DropKind::Map {
                key_type: Idx::INT,
                value_type: Idx::STR,
                dec_keys: false,
                dec_values: true,
            }
        );
    }

    #[test]
    fn map_str_keys_str_values() {
        let mut pool = Pool::new();
        let map = pool.map(Idx::STR, Idx::STR);
        let c = cls(&pool);

        let info = compute_drop_info(map, &c).unwrap();
        assert_eq!(
            info.kind,
            DropKind::Map {
                key_type: Idx::STR,
                value_type: Idx::STR,
                dec_keys: true,
                dec_values: true,
            }
        );
    }

    // ── Struct ─────────────────────────────────────────────────

    #[test]
    fn struct_with_one_rc_field() {
        let mut pool = Pool::new();
        let s = pool.struct_type(
            Name::from_raw(30),
            &[
                (Name::from_raw(31), Idx::INT),
                (Name::from_raw(32), Idx::STR),
            ],
        );
        let c = cls(&pool);

        let info = compute_drop_info(s, &c).unwrap();
        assert_eq!(info.kind, DropKind::Fields(vec![(1, Idx::STR)]));
    }

    #[test]
    fn struct_with_multiple_rc_fields() {
        let mut pool = Pool::new();
        let list_int = pool.list(Idx::INT);
        let s = pool.struct_type(
            Name::from_raw(40),
            &[
                (Name::from_raw(41), Idx::STR),
                (Name::from_raw(42), Idx::INT),
                (Name::from_raw(43), list_int),
            ],
        );
        let c = cls(&pool);

        let info = compute_drop_info(s, &c).unwrap();
        assert_eq!(
            info.kind,
            DropKind::Fields(vec![(0, Idx::STR), (2, list_int)])
        );
    }

    // ── Tuple ──────────────────────────────────────────────────

    #[test]
    fn tuple_with_rc_element() {
        let mut pool = Pool::new();
        let tup = pool.tuple(&[Idx::INT, Idx::STR]);
        let c = cls(&pool);

        let info = compute_drop_info(tup, &c).unwrap();
        assert_eq!(info.kind, DropKind::Fields(vec![(1, Idx::STR)]));
    }

    #[test]
    fn tuple_all_rc_elements() {
        let mut pool = Pool::new();
        let list_int = pool.list(Idx::INT);
        let tup = pool.tuple(&[Idx::STR, list_int]);
        let c = cls(&pool);

        let info = compute_drop_info(tup, &c).unwrap();
        assert_eq!(
            info.kind,
            DropKind::Fields(vec![(0, Idx::STR), (1, list_int)])
        );
    }

    // ── Enum ───────────────────────────────────────────────────

    #[test]
    fn enum_with_rc_variant_fields() {
        let mut pool = Pool::new();
        let e = pool.enum_type(
            Name::from_raw(50),
            &[
                EnumVariant {
                    name: Name::from_raw(51),
                    field_types: vec![Idx::INT],
                },
                EnumVariant {
                    name: Name::from_raw(52),
                    field_types: vec![Idx::STR],
                },
            ],
        );
        let c = cls(&pool);

        let info = compute_drop_info(e, &c).unwrap();
        assert_eq!(
            info.kind,
            DropKind::Enum(vec![
                vec![],              // variant 0: int field — scalar
                vec![(0, Idx::STR)], // variant 1: str field — needs Dec
            ])
        );
    }

    #[test]
    fn enum_with_mixed_variant_fields() {
        let mut pool = Pool::new();
        let list_str = pool.list(Idx::STR);
        let e = pool.enum_type(
            Name::from_raw(60),
            &[
                EnumVariant {
                    name: Name::from_raw(61),
                    field_types: vec![],
                },
                EnumVariant {
                    name: Name::from_raw(62),
                    field_types: vec![Idx::STR, Idx::INT],
                },
                EnumVariant {
                    name: Name::from_raw(63),
                    field_types: vec![list_str],
                },
            ],
        );
        let c = cls(&pool);

        let info = compute_drop_info(e, &c).unwrap();
        assert_eq!(
            info.kind,
            DropKind::Enum(vec![
                vec![],              // variant 0: unit
                vec![(0, Idx::STR)], // variant 1: str field at 0
                vec![(0, list_str)], // variant 2: list field at 0
            ])
        );
    }

    #[test]
    fn enum_all_scalar_payloads_returns_none() {
        let mut pool = Pool::new();
        let e = pool.enum_type(
            Name::from_raw(70),
            &[
                EnumVariant {
                    name: Name::from_raw(71),
                    field_types: vec![Idx::INT],
                },
                EnumVariant {
                    name: Name::from_raw(72),
                    field_types: vec![Idx::FLOAT],
                },
            ],
        );
        let c = cls(&pool);

        // The enum itself is Scalar because all payloads are scalar.
        assert!(compute_drop_info(e, &c).is_none());
    }

    // ── Option ─────────────────────────────────────────────────

    #[test]
    fn option_str_is_enum_drop() {
        let mut pool = Pool::new();
        let opt = pool.option(Idx::STR);
        let c = cls(&pool);

        let info = compute_drop_info(opt, &c).unwrap();
        assert_eq!(
            info.kind,
            DropKind::Enum(vec![
                vec![],              // None
                vec![(0, Idx::STR)], // Some(str)
            ])
        );
    }

    // ── Result ─────────────────────────────────────────────────

    #[test]
    fn result_str_int_drops_ok_only() {
        let mut pool = Pool::new();
        let res = pool.result(Idx::STR, Idx::INT);
        let c = cls(&pool);

        let info = compute_drop_info(res, &c).unwrap();
        assert_eq!(
            info.kind,
            DropKind::Enum(vec![
                vec![(0, Idx::STR)], // Ok(str) — needs Dec
                vec![],              // Err(int) — scalar
            ])
        );
    }

    #[test]
    fn result_int_str_drops_err_only() {
        let mut pool = Pool::new();
        let res = pool.result(Idx::INT, Idx::STR);
        let c = cls(&pool);

        let info = compute_drop_info(res, &c).unwrap();
        assert_eq!(
            info.kind,
            DropKind::Enum(vec![
                vec![],              // Ok(int) — scalar
                vec![(0, Idx::STR)], // Err(str) — needs Dec
            ])
        );
    }

    #[test]
    fn result_str_str_drops_both() {
        let mut pool = Pool::new();
        let res = pool.result(Idx::STR, Idx::STR);
        let c = cls(&pool);

        let info = compute_drop_info(res, &c).unwrap();
        assert_eq!(
            info.kind,
            DropKind::Enum(vec![
                vec![(0, Idx::STR)], // Ok(str)
                vec![(0, Idx::STR)], // Err(str)
            ])
        );
    }

    // ── Channel ────────────────────────────────────────────────

    #[test]
    fn channel_is_trivial() {
        let mut pool = Pool::new();
        let chan = pool.channel(Idx::INT);
        let c = cls(&pool);

        let info = compute_drop_info(chan, &c).unwrap();
        assert_eq!(info.kind, DropKind::Trivial);
    }

    // ── Function ───────────────────────────────────────────────

    #[test]
    fn function_is_trivial() {
        let mut pool = Pool::new();
        let func = pool.function(&[Idx::INT], Idx::STR);
        let c = cls(&pool);

        let info = compute_drop_info(func, &c).unwrap();
        assert_eq!(info.kind, DropKind::Trivial);
    }

    // ── Named type resolution ──────────────────────────────────

    #[test]
    fn named_type_resolves_to_struct_drop() {
        let mut pool = Pool::new();
        let name = Name::from_raw(80);
        let named_idx = pool.named(name);
        let struct_idx = pool.struct_type(
            name,
            &[
                (Name::from_raw(81), Idx::STR),
                (Name::from_raw(82), Idx::INT),
            ],
        );
        pool.set_resolution(named_idx, struct_idx);
        let c = cls(&pool);

        let info = compute_drop_info(named_idx, &c).unwrap();
        assert_eq!(info.kind, DropKind::Fields(vec![(0, Idx::STR)]));
    }

    // ── Closure env drop ───────────────────────────────────────

    #[test]
    fn closure_env_all_scalar() {
        let pool = Pool::new();
        let c = cls(&pool);

        let kind = compute_closure_env_drop(&[Idx::INT, Idx::FLOAT], &c);
        assert_eq!(kind, DropKind::Trivial);
    }

    #[test]
    fn closure_env_with_rc_captures() {
        let mut pool = Pool::new();
        let list_int = pool.list(Idx::INT);
        let c = cls(&pool);

        let kind = compute_closure_env_drop(&[Idx::INT, Idx::STR, list_int], &c);
        assert_eq!(
            kind,
            DropKind::ClosureEnv(vec![(1, Idx::STR), (2, list_int)])
        );
    }

    #[test]
    fn closure_env_single_rc_capture() {
        let pool = Pool::new();
        let c = cls(&pool);

        let kind = compute_closure_env_drop(&[Idx::STR], &c);
        assert_eq!(kind, DropKind::ClosureEnv(vec![(0, Idx::STR)]));
    }

    // ── collect_drop_infos ─────────────────────────────────────

    #[test]
    fn collect_from_empty_functions() {
        let pool = Pool::new();
        let c = cls(&pool);

        let infos = collect_drop_infos(&[], &c);
        assert!(infos.is_empty());
    }

    #[test]
    fn collect_deduplicates_types() {
        let pool = Pool::new();
        let c = cls(&pool);

        // Two RcDec instructions for the same type (str) → one DropInfo.
        let func = ArcFunction {
            name: Name::from_raw(100),
            params: vec![ArcParam {
                var: ArcVarId::new(0),
                ty: Idx::STR,
                ownership: Ownership::Owned,
            }],
            return_type: Idx::UNIT,
            blocks: vec![ArcBlock {
                id: ArcBlockId::new(0),
                params: vec![],
                body: vec![
                    ArcInstr::RcDec {
                        var: ArcVarId::new(0),
                    },
                    ArcInstr::RcDec {
                        var: ArcVarId::new(0),
                    },
                ],
                terminator: ArcTerminator::Unreachable,
            }],
            entry: ArcBlockId::new(0),
            var_types: vec![Idx::STR],
            spans: vec![vec![None, None]],
        };

        let infos = collect_drop_infos(&[func], &c);
        assert_eq!(infos.len(), 1);
        assert_eq!(infos[0].ty, Idx::STR);
        assert_eq!(infos[0].kind, DropKind::Trivial);
    }

    #[test]
    fn collect_multiple_types() {
        let mut pool = Pool::new();
        let list_str = pool.list(Idx::STR);
        let c = cls(&pool);

        let func = ArcFunction {
            name: Name::from_raw(110),
            params: vec![
                ArcParam {
                    var: ArcVarId::new(0),
                    ty: Idx::STR,
                    ownership: Ownership::Owned,
                },
                ArcParam {
                    var: ArcVarId::new(1),
                    ty: list_str,
                    ownership: Ownership::Owned,
                },
            ],
            return_type: Idx::UNIT,
            blocks: vec![ArcBlock {
                id: ArcBlockId::new(0),
                params: vec![],
                body: vec![
                    ArcInstr::RcDec {
                        var: ArcVarId::new(0),
                    },
                    ArcInstr::RcDec {
                        var: ArcVarId::new(1),
                    },
                ],
                terminator: ArcTerminator::Unreachable,
            }],
            entry: ArcBlockId::new(0),
            var_types: vec![Idx::STR, list_str],
            spans: vec![vec![None, None]],
        };

        let infos = collect_drop_infos(&[func], &c);
        assert_eq!(infos.len(), 2);

        // First: str (Trivial), Second: [str] (Collection)
        let str_info = infos.iter().find(|i| i.ty == Idx::STR).unwrap();
        assert_eq!(str_info.kind, DropKind::Trivial);

        let list_info = infos.iter().find(|i| i.ty == list_str).unwrap();
        assert_eq!(
            list_info.kind,
            DropKind::Collection {
                element_type: Idx::STR
            }
        );
    }

    #[test]
    fn collect_skips_scalar_rc_dec() {
        let pool = Pool::new();
        let c = cls(&pool);

        // RcDec on an int variable — should be skipped (classifier says no RC).
        let func = ArcFunction {
            name: Name::from_raw(120),
            params: vec![ArcParam {
                var: ArcVarId::new(0),
                ty: Idx::INT,
                ownership: Ownership::Owned,
            }],
            return_type: Idx::UNIT,
            blocks: vec![ArcBlock {
                id: ArcBlockId::new(0),
                params: vec![],
                body: vec![ArcInstr::RcDec {
                    var: ArcVarId::new(0),
                }],
                terminator: ArcTerminator::Unreachable,
            }],
            entry: ArcBlockId::new(0),
            var_types: vec![Idx::INT],
            spans: vec![vec![None]],
        };

        let infos = collect_drop_infos(&[func], &c);
        assert!(infos.is_empty());
    }

    // ── Nested compound types ──────────────────────────────────

    #[test]
    fn struct_with_nested_option_str_field() {
        let mut pool = Pool::new();
        let opt_str = pool.option(Idx::STR);
        let s = pool.struct_type(
            Name::from_raw(130),
            &[
                (Name::from_raw(131), Idx::INT),
                (Name::from_raw(132), opt_str),
            ],
        );
        let c = cls(&pool);

        let info = compute_drop_info(s, &c).unwrap();
        // Field 1 is option[str] which is DefiniteRef → needs Dec.
        assert_eq!(info.kind, DropKind::Fields(vec![(1, opt_str)]));
    }

    #[test]
    fn result_of_scalars_returns_none() {
        let mut pool = Pool::new();
        let res = pool.result(Idx::INT, Idx::FLOAT);
        let c = cls(&pool);

        // result[int, float] is Scalar → no drop needed.
        assert!(compute_drop_info(res, &c).is_none());
    }
}
