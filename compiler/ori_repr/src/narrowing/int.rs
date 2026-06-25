//! Integer narrowing — Phase A: struct/tuple field narrowing.
//!
//! Reads field-range summaries from `ReprPlan` (populated by range analysis) and
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

use super::{commit_narrowing_decision, is_narrowing_candidate};
use crate::plan::{DecisionReason, DecisionSource, ReprPlan};
use crate::repr::{IntWidth, MachineRepr};
use crate::struct_repr::FieldRepr;

/// Apply integer narrowing to struct and tuple fields in the `ReprPlan`.
///
/// Iterates all types with `Struct` or `Tuple` representations, checks
/// each `Int { I64, signed: true }` field against the field-range summary,
/// and narrows the width when the range fits in a smaller type.
///
/// **Narrowing/layout interface contract:** This pass writes only `FieldRepr.repr`;
/// `FieldRepr.offset` remains zero (the layout pass is the authority for offsets).
pub(crate) fn narrow_struct_fields(plan: &mut ReprPlan, pool: &Pool) {
    let policy = plan.narrowing_policy();
    if policy == crate::plan::NarrowingPolicy::Disabled {
        return;
    }

    let mut narrowed_count: u32 = 0;

    // Collect type indices that have Struct or Tuple reprs.
    // Collect first because mutable access to the plan is needed below.
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
        // assume canonical field widths. Tuple narrowing requires
        // element_store_size integration.
        if matches!(kind, CandidateKind::Tuple) {
            tracing::trace!(?idx, "skipping narrowing — tuple (Phase C)");
            continue;
        }

        // Skip types ineligible for narrowing (fixed layout / public ABI).
        if !is_narrowing_candidate(plan, idx) {
            tracing::trace!(?idx, "skipping narrowing — not a candidate");
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
                    commit_narrowing_decision(
                        plan,
                        pool,
                        idx,
                        DecisionSource::IntegerNarrowing,
                        MachineRepr::Struct(struct_repr),
                        DecisionReason::Custom(range_info),
                    );
                }
            }
            (CandidateKind::Tuple, MachineRepr::Tuple(mut tuple_repr)) => {
                let changed = narrow_fields(&mut tuple_repr.elements, idx, plan);
                if changed {
                    narrowed_count += 1;
                    let range_info = field_range_summary_string(idx, &tuple_repr.elements, plan);
                    commit_narrowing_decision(
                        plan,
                        pool,
                        idx,
                        DecisionSource::IntegerNarrowing,
                        MachineRepr::Tuple(tuple_repr),
                        DecisionReason::Custom(range_info),
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
/// **DISABLED**: Collection element narrowing is unsound when
/// `collect()` produces lists with computed values that exceed the narrowed
/// range. The narrowing analysis bases its decision on literal construction
/// sites (e.g., `[1,2,3]` fits in i8), but `iter().map(x -> x * 1000).collect()`
/// produces values (1000, 2000, ...) that exceed i8. Since all `List<int>`
/// share one `ReprPlan` entry, readers (equality, hash, display) and writers
/// (literal construction, collect, indexing) must agree on ONE stride. With
/// collect using canonical stride and the analysis only seeing literals,
/// narrowing creates a stride mismatch: silent data corruption in equality,
/// comparison, hashing, and display of collected lists.
///
/// Collection element narrowing is disabled until the analysis accounts for
/// ALL value sources (including collect output from iterator pipelines with
/// map/filter/etc. that can produce arbitrary values). Struct field narrowing
/// and local variable narrowing are unaffected.
///
/// Prior to this fix, sets were already excluded for a similar reason (eq/hash
/// thunks load canonical-width values from element pointers).
pub(crate) fn narrow_collection_elements(_plan: &mut ReprPlan, _pool: &Pool) {
    // disabled — see doc comment above.
    // When the narrowing analysis is extended to account for collect() output
    // values (not just literal construction sites), this can be re-enabled.
}
