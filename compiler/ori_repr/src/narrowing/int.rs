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

use ori_types::{Idx, Pool};

use crate::plan::{DecisionReason, DecisionSource, ReprDecision, ReprPlan};
use crate::repr::{IntWidth, MachineRepr};
use crate::struct_repr::FieldRepr;

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

    let _ = pool; // Pool unused in Phase A; reserved for Phase B/C.
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
                    plan.set_repr(
                        idx,
                        ReprDecision {
                            source: DecisionSource::IntegerNarrowing,
                            type_idx: idx,
                            repr: MachineRepr::Struct(struct_repr),
                            reason: DecisionReason::Custom(range_info),
                        },
                    );
                }
            }
            (CandidateKind::Tuple, MachineRepr::Tuple(mut tuple_repr)) => {
                let changed = narrow_fields(&mut tuple_repr.elements, idx, plan);
                if changed {
                    narrowed_count += 1;
                    let range_info = field_range_summary_string(idx, &tuple_repr.elements, plan);
                    plan.set_repr(
                        idx,
                        ReprDecision {
                            source: DecisionSource::IntegerNarrowing,
                            type_idx: idx,
                            repr: MachineRepr::Tuple(tuple_repr),
                            reason: DecisionReason::Custom(range_info),
                        },
                    );
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
