//! Signed-integer field narrowing driven by [`ReprPlan`] range summaries.

use ori_types::{Idx, Pool};

use crate::plan::{DecisionReason, DecisionSource, ReprPlan};
use crate::repr::{IntWidth, MachineRepr};
use crate::struct_repr::FieldRepr;

use super::super::{commit_narrowing_decision, is_narrowing_candidate};

/// Narrows eligible named-struct fields whose range fits a smaller signed width.
///
/// This pass only updates [`FieldRepr::repr`]; layout remains the sole owner of
/// [`FieldRepr::offset`]. Tuple fields remain canonical because collection and
/// iterator consumers assume canonical element widths.
pub(crate) fn narrow_struct_fields(plan: &mut ReprPlan, pool: &Pool) {
    let policy = plan.narrowing_policy();
    if policy == crate::plan::NarrowingPolicy::Disabled {
        return;
    }

    let mut narrowed_count: u32 = 0;

    // Why: Candidate discovery borrows the plan, while decisions mutate it.
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
        // Why: Tuple consumers and storage helpers assume canonical field widths.
        if matches!(kind, CandidateKind::Tuple) {
            tracing::trace!(?idx, "skipping narrowing — tuple (Phase C)");
            continue;
        }

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

/// Returns whether any canonical signed field gained a narrower representation.
fn narrow_fields(fields: &mut [FieldRepr], type_idx: Idx, plan: &ReprPlan) -> bool {
    let mut any_changed = false;

    for field in fields.iter_mut() {
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

fn is_canonical_int(repr: &MachineRepr) -> bool {
    matches!(
        repr,
        MachineRepr::Int {
            width: IntWidth::I64,
            signed: true
        }
    )
}

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
