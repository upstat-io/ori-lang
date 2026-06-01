//! Bridge between [`UserBurdenSpec`] (the AIMS pre-pass typed input) and the
//! shipped [`DropInfo`] enum consumed by per-type drop-function codegen.
//!
//! Spec: Annex E §AIMS — [`UserBurdenSpec`] is the canonical drop-walk /
//! reuse pre-pass input. This module owns the *forward* conversion from
//! [`UserBurdenSpec`] to the legacy [`DropInfo`] shape so consumers
//! ([`super::collect_drop_infos`] + the cross-crate
//! `ori_llvm::codegen::arc_emitter::element_fn_gen`) keep the existing
//! surface during the burden-pre-pass migration window.
//!
//! Inverse direction (synthesizing a [`UserBurdenSpec`] from the pool / tag
//! walk that the legacy [`super::compute_drop_info`] performed) lives in
//! [`synthesize_burden_from_pool`]. The two halves compose to make the lift
//! lossless on every type the 25 existing tests exercise.
//!
//! Mapping rules:
//! - `self_heap_alloc=false` + empty `owned_fields` + empty
//!   `variant_burdens` + no `element_burden` ⟹ [`DropKind::Trivial`].
//! - aggregate struct / tuple `OwnedField[]` ⟹ [`DropKind::Fields`].
//! - `VariantBurden[]` ⟹ [`DropKind::Enum`].
//! - `Shape::CollectionBuffer` over `T` (encoded as
//!   `element_burden = Some(T)` with `self_heap_alloc = true` and no
//!   `owned_fields`) ⟹ [`DropKind::Collection`].
//! - Map-shaped burden (encoded by [`synthesize_burden_from_pool`] with a
//!   sentinel two-element `owned_fields` carrying `(key_path=[0], K)` and
//!   `(value_path=[1], V)`) ⟹ [`DropKind::Map`].
//! - Closure-environment aggregate ([`SynthesizedShape::ClosureEnv`])
//!   ⟹ [`DropKind::ClosureEnv`].
//!
//! Soundness gate: every type currently exercised by `drop/tests.rs` must
//! round-trip through [`synthesize_burden_from_pool`] →
//! [`burden_to_drop_info`] producing the *same* [`DropInfo`] shape the
//! pre-migration `compute_drop_kind` produced (semantic pin:
//! `drop_info_via_burden_matches_legacy`).

use ori_registry::burden::TransferKind;
use ori_types::burden::{UserBurdenSpec, UserOwnedField, UserTransferRule, UserVariantBurden};
use ori_types::{Idx, Pool, Tag};

use crate::ArcClassification;

use super::{DropInfo, DropKind};

/// Discriminator stored alongside the synthesized `UserBurdenSpec` so the
/// forward converter can tell apart shapes that share the same field-list
/// layout in `UserBurdenSpec` (e.g. `Fields` vs `ClosureEnv` both project to
/// `owned_fields: Vec<UserOwnedField>`, and `Map` requires extra (`dec_keys`,
/// `dec_values`) signal not present in `UserBurdenSpec` itself).
///
/// `UserBurdenSpec` is the shared SSOT shape consumed by Phase 5 emission;
/// this discriminator is local to the drop bridge and never leaves the
/// module. Adding fields to `UserBurdenSpec` to carry this information would
/// be a `UserBurdenSpec` schema change — out of scope for the bridge's
/// wrapper-transparency invariant.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum SynthesizedShape {
    /// Aggregate struct / tuple — `owned_fields` lists each RC'd field in
    /// declaration order; emitted as `DropKind::Fields`.
    Fields,
    /// Sum type — `variant_burdens` carries one entry per source variant;
    /// emitted as `DropKind::Enum`.
    Enum,
    /// Collection backing buffer — `element_burden` carries the element
    /// type; emitted as `DropKind::Collection`.
    Collection { element_type: Idx },
    /// Map — `owned_fields` carries `[(path=[0], K), (path=[1], V)]` with
    /// only the RC'd halves populated; emitted as `DropKind::Map`.
    Map {
        key_type: Idx,
        value_type: Idx,
        dec_keys: bool,
        dec_values: bool,
    },
    /// Closure environment — same field-list layout as `Fields` but
    /// emitted as `DropKind::ClosureEnv` so codegen names the function
    /// `_ori_drop$__lambda_N_env`.
    ClosureEnv,
}

/// `UserBurdenSpec` synthesized from a pool walk plus the local
/// discriminator that `burden_to_drop_info` consumes to pick the correct
/// `DropKind` variant. Lifetime-free: callers may construct one per type
/// and discard it after the conversion.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct SynthesizedBurden {
    pub(super) spec: UserBurdenSpec,
    pub(super) shape: SynthesizedShape,
}

/// Convert a `SynthesizedBurden` into a `DropInfo` describing the same
/// drop-walk shape. Returns `None` when the spec is fully trivial (no
/// owned fields, no variants, no element, no map halves) — the caller
/// (legacy `compute_drop_info`) then returns `None` and codegen emits
/// no per-type drop function.
///
/// The `ty` parameter is the pool index the drop info describes; it
/// becomes `DropInfo.ty` verbatim and is not consulted otherwise.
pub(super) fn burden_to_drop_info(ty: Idx, synthesized: &SynthesizedBurden) -> Option<DropInfo> {
    let kind = burden_to_drop_kind(synthesized)?;
    Some(DropInfo { ty, kind })
}

/// Convert just the kind. Separated so callers that have already built a
/// `DropInfo` shell can replace the kind without copying the type index.
fn burden_to_drop_kind(synthesized: &SynthesizedBurden) -> Option<DropKind> {
    match &synthesized.shape {
        SynthesizedShape::Collection { element_type } => Some(DropKind::Collection {
            element_type: *element_type,
        }),
        SynthesizedShape::Map {
            key_type,
            value_type,
            dec_keys,
            dec_values,
        } => Some(DropKind::Map {
            key_type: *key_type,
            value_type: *value_type,
            dec_keys: *dec_keys,
            dec_values: *dec_values,
        }),
        SynthesizedShape::Fields => {
            let fields = owned_fields_to_pairs(&synthesized.spec.owned_fields);
            // Always Some when user_drop is set — the user @drop body
            // needs an entry point invoked at refcount-zero even when
            // there are no RC'd fields to walk. Without this, a Drop
            // type with all-scalar fields would silently bypass @drop.
            if fields.is_empty() && synthesized.spec.user_drop.is_none() {
                None
            } else {
                Some(DropKind::Fields {
                    fields,
                    user_drop: synthesized.spec.user_drop,
                })
            }
        }
        SynthesizedShape::ClosureEnv => {
            let fields = owned_fields_to_pairs(&synthesized.spec.owned_fields);
            if fields.is_empty() {
                Some(DropKind::Trivial)
            } else {
                // Closures cannot carry user @drop — only nominal types
                // can implement Drop (per `drop-trait-proposal.md
                // §Definition`). user_drop stays None on ClosureEnv.
                Some(DropKind::ClosureEnv(fields))
            }
        }
        SynthesizedShape::Enum => {
            let variants: Vec<Vec<(u32, Idx)>> = synthesized
                .spec
                .variant_burdens
                .iter()
                .map(|v| owned_fields_to_pairs(&v.retained_owned))
                .collect();
            // Same Drop-aware emit rule as Fields: emit Enum even when
            // every variant is empty when user_drop is set so codegen
            // materializes the AUGMENT body.
            if variants.iter().all(Vec::is_empty) && synthesized.spec.user_drop.is_none() {
                None
            } else {
                Some(DropKind::Enum {
                    variants,
                    user_drop: synthesized.spec.user_drop,
                })
            }
        }
    }
}

fn owned_fields_to_pairs(fields: &[UserOwnedField]) -> Vec<(u32, Idx)> {
    fields
        .iter()
        .filter_map(|f| f.field_path.first().map(|idx| (*idx, f.field_type)))
        .collect()
}

/// Synthesize a `UserBurdenSpec` (plus its `SynthesizedShape` discriminator)
/// from the pool / tag walk a caller would otherwise perform inline.
///
/// Returns `None` for types whose drop is trivial OR genuinely empty —
/// scalars, all-scalar aggregates, iterators (handled inline by the ARC
/// emitter via `RcStrategy::Iterator`, never by per-type drop functions).
///
/// Mirrors the pre-migration `compute_drop_kind`'s tag dispatch exactly so
/// the burden-bridge lift preserves the 25 existing tests' observable shapes.
/// The wrapper at [`super::compute_drop_info`] composes this with
/// [`burden_to_drop_info`] to realise the conversion.
pub(super) fn synthesize_burden_from_pool(
    ty: Idx,
    classifier: &dyn ArcClassification,
    pool: &Pool,
) -> Option<SynthesizedBurden> {
    let (resolved_ty, resolved_tag) = resolve_type(ty, pool);
    match resolved_tag {
        Tag::List => synthesize_collection(pool.list_elem(resolved_ty), classifier),
        Tag::Set => synthesize_collection(pool.set_elem(resolved_ty), classifier),
        Tag::Map => synthesize_map(
            pool.map_key(resolved_ty),
            pool.map_value(resolved_ty),
            classifier,
        ),
        Tag::Struct => synthesize_fields(
            pool.struct_fields(resolved_ty)
                .into_iter()
                .map(|(_, ty)| ty),
            classifier,
            SynthesizedShape::Fields,
        ),
        Tag::Tuple => synthesize_fields(
            pool.tuple_elems(resolved_ty).into_iter(),
            classifier,
            SynthesizedShape::Fields,
        ),
        Tag::Enum => synthesize_enum(
            pool.enum_variants(resolved_ty)
                .into_iter()
                .map(|(_, field_types)| field_types),
            classifier,
        ),
        Tag::Option => synthesize_option(pool.option_inner(resolved_ty), classifier),
        Tag::Result => synthesize_result(
            pool.result_ok(resolved_ty),
            pool.result_err(resolved_ty),
            classifier,
        ),
        Tag::Range => synthesize_range(pool.range_elem(resolved_ty), classifier),
        // Trivial fallback: Named / Applied / Alias resolution is
        // exhausted by `resolve_type` above; primitives and stack types
        // fall here too. Returning `None` delegates the
        // `DropKind::Trivial` vs `None` decision to the caller.
        _ => None,
    }
}

fn synthesize_collection(
    elem: Idx,
    classifier: &dyn ArcClassification,
) -> Option<SynthesizedBurden> {
    if classifier.needs_rc(elem) {
        Some(make_collection(elem))
    } else {
        None
    }
}

fn synthesize_map(
    key: Idx,
    value: Idx,
    classifier: &dyn ArcClassification,
) -> Option<SynthesizedBurden> {
    let dk = classifier.needs_rc(key);
    let dv = classifier.needs_rc(value);
    if dk || dv {
        Some(make_map(key, value, dk, dv))
    } else {
        None
    }
}

fn synthesize_option(inner: Idx, classifier: &dyn ArcClassification) -> Option<SynthesizedBurden> {
    if !classifier.needs_rc(inner) {
        return None;
    }
    let variants = vec![
        UserVariantBurden {
            variant_id: variant_id_at(0),
            transfers_on_match: Vec::new(),
            retained_owned: Vec::new(),
        },
        UserVariantBurden {
            variant_id: variant_id_at(1),
            transfers_on_match: vec![transfer_rule_for(0, inner)],
            retained_owned: vec![owned_field_at(0, inner)],
        },
    ];
    Some(make_enum(variants))
}

fn synthesize_result(
    ok_ty: Idx,
    err_ty: Idx,
    classifier: &dyn ArcClassification,
) -> Option<SynthesizedBurden> {
    let ok_rc = classifier.needs_rc(ok_ty);
    let err_rc = classifier.needs_rc(err_ty);
    if !ok_rc && !err_rc {
        return None;
    }
    let variants = vec![
        variant_for_arm(0, ok_ty, ok_rc),
        variant_for_arm(1, err_ty, err_rc),
    ];
    Some(make_enum(variants))
}

fn variant_for_arm(ord: usize, ty: Idx, needs_rc: bool) -> UserVariantBurden {
    UserVariantBurden {
        variant_id: variant_id_at(ord),
        transfers_on_match: if needs_rc {
            vec![transfer_rule_for(0, ty)]
        } else {
            Vec::new()
        },
        retained_owned: if needs_rc {
            vec![owned_field_at(0, ty)]
        } else {
            Vec::new()
        },
    }
}

fn synthesize_range(elem: Idx, classifier: &dyn ArcClassification) -> Option<SynthesizedBurden> {
    if !classifier.needs_rc(elem) {
        return None;
    }
    // Range has start and end fields of the same type — mirror the
    // pre-migration `compute_fields_drop`'s emit of `(0, elem), (1, elem)`.
    let owned_fields = vec![owned_field_at(0, elem), owned_field_at(1, elem)];
    Some(SynthesizedBurden {
        spec: UserBurdenSpec {
            self_heap_alloc: false,
            owned_fields,
            borrowed_fields: Vec::new(),
            variant_burdens: Vec::new(),
            element_burden: None,
            compiled_drop: None,
            user_drop: None,
        },
        shape: SynthesizedShape::Fields,
    })
}

/// Builds a `SynthesizedBurden` for a fixed-layout aggregate (struct /
/// tuple). Closure environments share the field-list layout but emit as
/// `DropKind::ClosureEnv`; pass `SynthesizedShape::ClosureEnv` to override.
fn synthesize_fields(
    field_types: impl Iterator<Item = Idx>,
    classifier: &dyn ArcClassification,
    shape: SynthesizedShape,
) -> Option<SynthesizedBurden> {
    let owned_fields: Vec<UserOwnedField> = field_types
        .enumerate()
        .filter_map(|(i, field_ty)| {
            if classifier.needs_rc(field_ty) {
                #[expect(
                    clippy::cast_possible_truncation,
                    reason = "field indices are bounded by struct layout, never exceed u32"
                )]
                Some(owned_field_at(i as u32, field_ty))
            } else {
                None
            }
        })
        .collect();

    if owned_fields.is_empty() {
        None
    } else {
        Some(SynthesizedBurden {
            spec: UserBurdenSpec {
                self_heap_alloc: false,
                owned_fields,
                borrowed_fields: Vec::new(),
                variant_burdens: Vec::new(),
                element_burden: None,
                compiled_drop: None,
                user_drop: None,
            },
            shape,
        })
    }
}

fn synthesize_enum(
    variants: impl Iterator<Item = Vec<Idx>>,
    classifier: &dyn ArcClassification,
) -> Option<SynthesizedBurden> {
    let variant_burdens: Vec<UserVariantBurden> = variants
        .enumerate()
        .map(|(variant_ord, field_types)| {
            let mut owned = Vec::new();
            let mut transfers = Vec::new();
            for (i, field_ty) in field_types.into_iter().enumerate() {
                if classifier.needs_rc(field_ty) {
                    #[expect(
                        clippy::cast_possible_truncation,
                        reason = "field indices are bounded by variant layout, never exceed u32"
                    )]
                    let field_index = i as u32;
                    owned.push(owned_field_at(field_index, field_ty));
                    transfers.push(transfer_rule_for(field_index, field_ty));
                }
            }
            UserVariantBurden {
                variant_id: variant_id_at(variant_ord),
                transfers_on_match: transfers,
                retained_owned: owned,
            }
        })
        .collect();

    if variant_burdens.iter().all(|v| v.retained_owned.is_empty()) {
        None
    } else {
        Some(make_enum(variant_burdens))
    }
}

fn make_collection(element_type: Idx) -> SynthesizedBurden {
    SynthesizedBurden {
        spec: UserBurdenSpec {
            self_heap_alloc: true,
            owned_fields: Vec::new(),
            borrowed_fields: Vec::new(),
            variant_burdens: Vec::new(),
            element_burden: Some(element_type),
            compiled_drop: None,
            user_drop: None,
        },
        shape: SynthesizedShape::Collection { element_type },
    }
}

fn make_map(key_type: Idx, value_type: Idx, dec_keys: bool, dec_values: bool) -> SynthesizedBurden {
    let mut owned_fields = Vec::new();
    if dec_keys {
        owned_fields.push(UserOwnedField {
            field_path: vec![0],
            field_type: key_type,
        });
    }
    if dec_values {
        owned_fields.push(UserOwnedField {
            field_path: vec![1],
            field_type: value_type,
        });
    }
    SynthesizedBurden {
        spec: UserBurdenSpec {
            self_heap_alloc: true,
            owned_fields,
            borrowed_fields: Vec::new(),
            variant_burdens: Vec::new(),
            element_burden: None,
            compiled_drop: None,
            user_drop: None,
        },
        shape: SynthesizedShape::Map {
            key_type,
            value_type,
            dec_keys,
            dec_values,
        },
    }
}

fn make_enum(variants: Vec<UserVariantBurden>) -> SynthesizedBurden {
    SynthesizedBurden {
        spec: UserBurdenSpec {
            self_heap_alloc: false,
            owned_fields: Vec::new(),
            borrowed_fields: Vec::new(),
            variant_burdens: variants,
            element_burden: None,
            compiled_drop: None,
            user_drop: None,
        },
        shape: SynthesizedShape::Enum,
    }
}

fn owned_field_at(field_index: u32, field_type: Idx) -> UserOwnedField {
    UserOwnedField {
        field_path: vec![field_index],
        field_type,
    }
}

fn transfer_rule_for(field_index: u32, field_type: Idx) -> UserTransferRule {
    UserTransferRule {
        source_field_path: vec![field_index],
        binding_index: 0,
        field_type,
        transfer_kind: TransferKind::Move,
    }
}

fn variant_id_at(variant_ord: usize) -> ori_registry::burden::VariantId {
    // `VariantId` wraps `NonZeroU32`; `variant_ord + 1` keeps the niche
    // (ord 0 → id 1, ord 1 → id 2, …).
    #[expect(
        clippy::cast_possible_truncation,
        reason = "variant ordinals are bounded by enum layout, never exceed u32"
    )]
    let n = (variant_ord as u32).saturating_add(1);
    let nz = core::num::NonZeroU32::new(n).unwrap_or(core::num::NonZeroU32::MIN);
    ori_registry::burden::VariantId::new(nz)
}

/// Synthesize a `SynthesizedBurden` for a closure environment from its
/// capture-type slice. Companion to [`synthesize_burden_from_pool`] for the
/// shape that has no Pool entry — closure environments are constructed by
/// `PartialApply` lowering at ARC-IR time, not interned in the type pool.
///
/// Returns `None` when no captured variable needs RC; the caller
/// (`compute_closure_env_drop`) emits `DropKind::Trivial`.
pub(super) fn synthesize_closure_env_burden(
    capture_types: &[Idx],
    classifier: &dyn ArcClassification,
) -> Option<SynthesizedBurden> {
    synthesize_fields(
        capture_types.iter().copied(),
        classifier,
        SynthesizedShape::ClosureEnv,
    )
}

/// Resolve a type through Named/Alias indirection to its concrete tag.
///
/// Mirror of `compute_drop_kind`'s helper (intentional algorithmic
/// duplication — the legacy function is also a consumer of this resolution
/// step, and centralising it would broaden the §02.3 lift past the wrapper
/// surface). When §06 lands the full BurdenRegistry-driven dispatch the
/// legacy `compute_drop_kind` retires and this helper can move up.
fn resolve_type(ty: Idx, pool: &Pool) -> (Idx, Tag) {
    // Out-of-bounds idx has no resolvable tag — opaque leaf (mirrors
    // `Pool::resolve_fully`'s bounds guard; `pool.tag` would panic).
    if ty.raw() as usize >= pool.len() {
        return (ty, Tag::Error);
    }
    let tag = pool.tag(ty);
    match tag {
        Tag::Named | Tag::Applied | Tag::Alias => match pool.resolve(ty) {
            Some(resolved) => resolve_type(resolved, pool),
            None => (ty, tag),
        },
        _ => (ty, tag),
    }
}
