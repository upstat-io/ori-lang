//! Integer narrowing — Phase A: struct/tuple field narrowing.
//!
//! Reads field-range summaries from `ReprPlan` (populated by §03) and
//! replaces `Int { width: I64 }` fields with narrower widths when the
//! range proves it safe.
//!
//! # Conservatism
//!
//! - `#repr("c")` / `#repr("packed")` types are **never** narrowed
//!   (ABI contract with external code)
//! - `#repr("transparent")` types are **never** narrowed (layout must
//!   match the single inner field)
//! - Only fields with `MachineRepr::Int { width: I64, signed: true }`
//!   are candidates; narrower widths are already optimal
//! - Fields with `Top` range are left at I64 (safe default)
//! - Fields with `Bottom` range get I8 (unreachable — smallest valid)

use ori_types::{Idx, Pool, Tag};

use crate::plan::{DecisionReason, DecisionSource, ReprDecision, ReprPlan};
use crate::repr::{IntWidth, MachineRepr};
use crate::struct_repr::{FatRepr, FieldRepr};

/// Apply integer narrowing to struct and tuple fields in the `ReprPlan`.
///
/// Iterates all types with `Struct` or `Tuple` representations, checks
/// each `Int { I64, signed: true }` field against the field-range summary,
/// and narrows the width when the range fits in a smaller type.
///
/// **§04/§06 interface contract:** This pass writes only `FieldRepr.repr`;
/// `FieldRepr.offset` remains zero (§06 is the authority for layout).
pub fn narrow_struct_fields(plan: &mut ReprPlan, pool: &Pool) {
    let policy = plan.narrowing_policy();
    if policy == crate::plan::NarrowingPolicy::Disabled {
        return;
    }

    let mut narrowed_count: u32 = 0;

    // Collect type indices that have Struct or Tuple reprs.
    // We must collect first because we need mutable access to the plan.
    let candidates: Vec<(Idx, CandidateKind)> = plan
        .decision_indices()
        .filter_map(|idx| {
            let repr = plan.get_repr(idx)?;
            match repr {
                MachineRepr::Struct(_) => Some((idx, CandidateKind::Struct)),
                MachineRepr::Tuple(_) => Some((idx, CandidateKind::Tuple)),
                _ => None,
            }
        })
        .collect();

    for (idx, kind) in candidates {
        // Phase A: skip tuples — only narrow named structs.
        // Tuples are used as collection elements, iterator state, and
        // intermediate values where element_store_size() / elem_dec_fn
        // assume canonical field widths. Tuple narrowing requires Phase C
        // element_store_size integration (§04.4).
        if matches!(kind, CandidateKind::Tuple) {
            tracing::trace!(?idx, "skipping narrowing — tuple (Phase C)");
            continue;
        }

        // Skip types with ABI-fixed layout attributes.
        if has_fixed_layout_attr(plan, idx) {
            tracing::trace!(?idx, "skipping narrowing — fixed layout attribute");
            continue;
        }

        // Skip public types — their field layout is an ABI contract
        // with external code (TPR-04-005).
        if plan.is_public_type(idx) {
            tracing::trace!(?idx, "skipping narrowing — public type (ABI contract)");
            continue;
        }

        let Some(repr) = plan.get_repr(idx).cloned() else {
            continue;
        };

        match (kind, repr) {
            (CandidateKind::Struct, MachineRepr::Struct(mut struct_repr)) => {
                let changed = narrow_fields(&mut struct_repr.fields, idx, plan);
                if changed {
                    narrowed_count += 1;
                    let range_info = field_range_summary_string(idx, &struct_repr.fields, plan);
                    let repr = MachineRepr::Struct(struct_repr);
                    let decision = ReprDecision {
                        source: DecisionSource::IntegerNarrowing,
                        type_idx: idx,
                        repr: repr.clone(),
                        reason: DecisionReason::Custom(range_info.clone()),
                    };
                    plan.set_repr(idx, decision);
                    // Codegen canonicalizes via pool.resolve_fully() — if the
                    // original idx (e.g., Named("Pixel")) resolves to a different
                    // concrete idx (e.g., Struct(fields)), store the narrowed
                    // decision there too. Without this, codegen queries the
                    // resolved idx and finds the original canonical (I64) version.
                    let resolved = pool.resolve_fully(idx);
                    if resolved != idx {
                        plan.set_repr(
                            resolved,
                            ReprDecision {
                                source: DecisionSource::IntegerNarrowing,
                                type_idx: resolved,
                                repr,
                                reason: DecisionReason::Custom(range_info),
                            },
                        );
                    }
                }
            }
            (CandidateKind::Tuple, MachineRepr::Tuple(mut tuple_repr)) => {
                let changed = narrow_fields(&mut tuple_repr.elements, idx, plan);
                if changed {
                    narrowed_count += 1;
                    let range_info = field_range_summary_string(idx, &tuple_repr.elements, plan);
                    let repr = MachineRepr::Tuple(tuple_repr);
                    let decision = ReprDecision {
                        source: DecisionSource::IntegerNarrowing,
                        type_idx: idx,
                        repr: repr.clone(),
                        reason: DecisionReason::Custom(range_info.clone()),
                    };
                    plan.set_repr(idx, decision);
                    let resolved = pool.resolve_fully(idx);
                    if resolved != idx {
                        plan.set_repr(
                            resolved,
                            ReprDecision {
                                source: DecisionSource::IntegerNarrowing,
                                type_idx: resolved,
                                repr,
                                reason: DecisionReason::Custom(range_info),
                            },
                        );
                    }
                }
            }
            _ => {}
        }
    }

    tracing::debug!(
        narrowed_count,
        "integer narrowing complete (struct/tuple fields)"
    );
}

/// Narrow int fields in a field list. Returns `true` if any field was narrowed.
fn narrow_fields(fields: &mut [FieldRepr], type_idx: Idx, plan: &ReprPlan) -> bool {
    let mut any_changed = false;

    for field in fields.iter_mut() {
        // Only narrow canonical i64 signed integers.
        if !is_canonical_int(&field.repr) {
            continue;
        }

        let range = plan.field_range(type_idx, field.original_index);
        let min_width = range.min_width();

        if min_width != IntWidth::I64 {
            tracing::debug!(
                ?type_idx,
                field_index = field.original_index,
                ?range,
                ?min_width,
                "narrowing struct field"
            );
            field.repr = MachineRepr::Int {
                width: min_width,
                signed: true,
            };
            any_changed = true;
        }
    }

    any_changed
}

/// Check if a `MachineRepr` is the canonical `Int { I64, signed: true }`.
fn is_canonical_int(repr: &MachineRepr) -> bool {
    matches!(
        repr,
        MachineRepr::Int {
            width: IntWidth::I64,
            signed: true
        }
    )
}

/// Check if a type has a fixed-layout attribute that prevents narrowing.
///
/// `#repr("c")`, `#repr("c", aligned N)`, `#repr("packed")`, and
/// `#repr("transparent")` all fix the field layout — narrowing would
/// violate the ABI contract. `#repr("aligned", N)` alone does NOT
/// prevent narrowing: it sets whole-struct alignment, not field layout.
fn has_fixed_layout_attr(plan: &ReprPlan, idx: Idx) -> bool {
    plan.repr_attr(idx).is_some_and(|attr| {
        matches!(
            attr,
            crate::plan::ReprAttribute::C
                | crate::plan::ReprAttribute::CAligned(_)
                | crate::plan::ReprAttribute::Packed
                | crate::plan::ReprAttribute::Transparent
        )
    })
}

/// Build a summary string of field ranges for the audit trail.
fn field_range_summary_string(idx: Idx, fields: &[FieldRepr], plan: &ReprPlan) -> String {
    use std::fmt::Write;
    let mut s = String::from("field narrowing: ");
    for (i, field) in fields.iter().enumerate() {
        if i > 0 {
            s.push_str(", ");
        }
        let range = plan.field_range(idx, field.original_index);
        let _ = write!(
            s,
            "f{}: {:?} → {:?}",
            field.original_index, range, field.repr
        );
    }
    s
}

#[derive(Clone, Copy)]
enum CandidateKind {
    Struct,
    Tuple,
}

/// Apply integer narrowing to collection element types (Phase C).
///
/// Iterates all types with `FatPointer(Collection { element_repr })` where
/// the element is canonical `Int { I64, signed: true }`. When the element
/// range (from `ElementSummaryTable`) fits in a smaller width, narrows the
/// `element_repr` field.
///
/// Conservatism rules (same as struct field narrowing):
/// - Public collection types are not narrowed (ABI contract)
/// - `NarrowingPolicy::Disabled` suppresses all narrowing
/// - Collections with no observed construction sites stay at I64 (`Top`)
/// - Only `[int]` is a candidate (maps and sets not yet supported)
///
/// Sets are excluded because the eq/hash thunks generated for set hash
/// table operations always load the canonical LLVM type (`i64` for `int`)
/// from element pointers. Narrowing set elements would cause the thunks
/// to read past the narrowed slot (e.g., reading 8 bytes from a 1-byte
/// element), producing garbage comparisons and hashes. Set narrowing
/// requires narrowing-aware thunks — tracked separately.
pub fn narrow_collection_elements(plan: &mut ReprPlan, pool: &Pool) {
    let policy = plan.narrowing_policy();
    if policy == crate::plan::NarrowingPolicy::Disabled {
        return;
    }

    let mut narrowed_count: u32 = 0;

    // Collect collection type indices with FatPointer(Collection) reprs
    // whose element type is canonical i64.
    let candidates: Vec<(Idx, FatRepr)> = plan
        .decision_indices()
        .filter_map(|idx| {
            let repr = plan.get_repr(idx)?;
            if let MachineRepr::FatPointer(fat @ FatRepr::Collection { ref element_repr }) = repr {
                if is_canonical_int(element_repr) {
                    return Some((idx, fat.clone()));
                }
            }
            None
        })
        .collect();

    for (idx, _fat) in candidates {
        // Skip sets — eq/hash thunks load canonical-width values from element
        // pointers; narrowing set elements causes them to read past the slot.
        if pool.tag(idx) == Tag::Set {
            tracing::trace!(
                ?idx,
                "skipping collection narrowing — set type (thunks not narrowing-aware)"
            );
            continue;
        }

        // Skip public collection types — ABI contract with external code.
        if plan.is_public_type(idx) {
            tracing::trace!(?idx, "skipping collection narrowing — public type");
            continue;
        }

        // Skip types with fixed layout attributes.
        if has_fixed_layout_attr(plan, idx) {
            tracing::trace!(?idx, "skipping collection narrowing — fixed layout attr");
            continue;
        }

        let element_range = plan.element_range(idx);
        let min_width = element_range.min_width();

        if min_width == IntWidth::I64 {
            continue;
        }

        tracing::debug!(
            ?idx,
            ?element_range,
            ?min_width,
            "narrowing collection element type"
        );

        let new_element_repr = MachineRepr::Int {
            width: min_width,
            signed: true,
        };
        let new_repr = MachineRepr::FatPointer(FatRepr::Collection {
            element_repr: Box::new(new_element_repr),
        });

        let reason = format!("element narrowing: {element_range:?} → {min_width:?}");
        let decision = ReprDecision {
            source: DecisionSource::IntegerNarrowing,
            type_idx: idx,
            repr: new_repr.clone(),
            reason: DecisionReason::Custom(reason.clone()),
        };
        plan.set_repr(idx, decision);

        // Also store for resolved idx (same pattern as struct narrowing).
        let resolved = pool.resolve_fully(idx);
        if resolved != idx {
            plan.set_repr(
                resolved,
                ReprDecision {
                    source: DecisionSource::IntegerNarrowing,
                    type_idx: resolved,
                    repr: new_repr,
                    reason: DecisionReason::Custom(reason),
                },
            );
        }

        narrowed_count += 1;
    }

    tracing::debug!(
        narrowed_count,
        "integer narrowing complete (collection elements)"
    );
}
