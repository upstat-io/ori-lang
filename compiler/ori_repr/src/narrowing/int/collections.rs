//! Integer narrowing for collection elements.

use ori_types::{Idx, Pool, Tag};

use crate::plan::{DecisionReason, DecisionSource, ReprPlan};
use crate::repr::{IntWidth, MachineRepr};
use crate::struct_repr::FatRepr;

use super::super::{commit_narrowing_decision, is_narrowing_candidate};

/// Narrows canonical `List<Int>` elements when every producer proves a bounded width.
///
/// Public, fixed-layout, unbounded, and non-list collection representations stay
/// canonical because their shared runtime stride and equality/hash thunks assume
/// `I64` elements.
pub(crate) fn narrow_collection_elements(plan: &mut ReprPlan, pool: &Pool) {
    if plan.narrowing_policy() == crate::plan::NarrowingPolicy::Disabled {
        return;
    }

    // Why: Candidate discovery borrows the plan, while decisions mutate it.
    let candidates: Vec<Idx> = plan
        .decision_indices()
        .filter(|&idx| {
            matches!(
                plan.get_repr(idx),
                Some(MachineRepr::FatPointer(FatRepr::Collection { element_repr }))
                    if is_canonical_int(element_repr)
            )
        })
        .collect();

    let mut narrowed_count: u32 = 0;

    for idx in candidates {
        let resolved = pool.resolve_fully(idx);

        // Why: Non-list collection thunks still load canonical-width elements.
        if pool.tag(resolved) != Tag::List {
            tracing::trace!(
                ?idx,
                ?resolved,
                "skipping collection narrowing — not a list"
            );
            continue;
        }

        if !is_narrowing_candidate(plan, idx)
            || (resolved != idx && !is_narrowing_candidate(plan, resolved))
        {
            tracing::trace!(?idx, "skipping collection narrowing — not a candidate");
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

        let new_repr = MachineRepr::FatPointer(FatRepr::Collection {
            element_repr: Box::new(MachineRepr::Int {
                width: min_width,
                signed: true,
            }),
        });
        let reason = format!("element narrowing: {element_range:?} → {min_width:?}");
        commit_narrowing_decision(
            plan,
            pool,
            idx,
            DecisionSource::IntegerNarrowing,
            new_repr,
            DecisionReason::Custom(reason),
        );
        narrowed_count += 1;
    }

    tracing::debug!(
        narrowed_count,
        "integer narrowing complete (collection elements)"
    );
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
