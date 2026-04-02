//! Alias propagation for struct/tuple layout reordering.
//!
//! When the struct layout pass reorders fields for padding minimization,
//! the new layout must propagate to all Pool entries that represent the
//! same logical type. Monomorphized generics create multiple `Idx` values
//! for structurally identical types — without propagation, call sites may
//! use a different `Idx` than the function body, causing layout mismatch.

use ori_types::{Idx, Pool, Tag};

use crate::plan::ReprPlan;
use crate::{DecisionReason, DecisionSource, MachineRepr, ReprDecision};

/// Propagate a reordered layout to all Pool entries that share the same
/// concrete type structure. This handles the Pool aliasing issue where
/// monomorphized generics create multiple Idx values for the same logical type.
pub(super) fn propagate_layout_to_aliases(
    plan: &mut ReprPlan,
    pool: &Pool,
    source_idx: Idx,
    repr: &MachineRepr,
) {
    let pool_len = u32::try_from(pool.len()).unwrap_or(u32::MAX);
    let source_tag = pool.tag(source_idx);

    // Only propagate tuples and structs.
    if source_tag != Tag::Tuple && source_tag != Tag::Struct {
        return;
    }

    // Get the structural signature of the source type (resolved element types).
    let source_elems: Vec<Idx> = if source_tag == Tag::Tuple {
        pool.tuple_elems(source_idx)
            .into_iter()
            .map(|e| pool.resolve_fully(e))
            .collect()
    } else {
        pool.struct_fields(source_idx)
            .into_iter()
            .map(|(_, ty)| pool.resolve_fully(ty))
            .collect()
    };

    // Scan for matching types that don't yet have the reordered layout.
    for raw in Idx::FIRST_DYNAMIC..pool_len {
        let idx = Idx::from_raw(raw);
        if idx == source_idx {
            continue;
        }
        if pool.tag(idx) != source_tag {
            continue;
        }
        // Check structural match using deep structural equality.
        // Plain Idx comparison fails for monomorphized generics where
        // nested composite children (e.g., inner tuples) get different
        // Pool Idx values despite being structurally identical.
        let elems: Vec<Idx> = if source_tag == Tag::Tuple {
            pool.tuple_elems(idx)
                .into_iter()
                .map(|e| pool.resolve_fully(e))
                .collect()
        } else {
            pool.struct_fields(idx)
                .into_iter()
                .map(|(_, ty)| pool.resolve_fully(ty))
                .collect()
        };
        if elems.len() == source_elems.len()
            && elems
                .iter()
                .zip(source_elems.iter())
                .all(|(&a, &b)| structural_type_eq(pool, a, b))
        {
            // For structs: verify field NAMES match in the same order.
            // Different Pool entries for the same struct can have fields
            // in different orders — the StructRepr's original_index values
            // assume the source's field order and would be wrong for a target
            // with different ordering.
            if source_tag == Tag::Struct {
                let source_names: Vec<_> = pool
                    .struct_fields(source_idx)
                    .into_iter()
                    .map(|(name, _)| name)
                    .collect();
                let target_names: Vec<_> = pool
                    .struct_fields(idx)
                    .into_iter()
                    .map(|(name, _)| name)
                    .collect();
                if source_names != target_names {
                    continue;
                }
                // Don't propagate to types with fixed-layout attrs.
                if let Some(attr) = plan.repr_attr(idx) {
                    if !matches!(attr, crate::plan::ReprAttribute::Default) {
                        continue;
                    }
                }
            }
            // Same structural type, compatible attributes and field order — propagate.
            plan.set_repr(
                idx,
                ReprDecision {
                    source: DecisionSource::StructLayout,
                    type_idx: idx,
                    repr: repr.clone(),
                    reason: DecisionReason::Custom("field reordering (alias propagation)".into()),
                },
            );
        }
    }
}

/// Deep structural type equality for alias propagation.
///
/// Two resolved Idx values are structurally equal if they have the same tag
/// and their children are structurally equal. This handles the case where
/// monomorphized generics create distinct Pool entries for the same logical
/// type (e.g., two `(int, int)` tuples from different generic instantiations).
fn structural_type_eq(pool: &Pool, a: Idx, b: Idx) -> bool {
    let a = pool.resolve_fully(a);
    let b = pool.resolve_fully(b);
    if a == b {
        return true;
    }

    let a_tag = pool.tag(a);
    let b_tag = pool.tag(b);
    if a_tag != b_tag {
        return false;
    }

    match a_tag {
        Tag::Tuple => {
            let a_elems = pool.tuple_elems(a);
            let b_elems = pool.tuple_elems(b);
            a_elems.len() == b_elems.len()
                && a_elems
                    .iter()
                    .zip(b_elems.iter())
                    .all(|(&x, &y)| structural_type_eq(pool, x, y))
        }
        Tag::Struct => {
            let a_fields = pool.struct_fields(a);
            let b_fields = pool.struct_fields(b);
            a_fields.len() == b_fields.len()
                && a_fields
                    .iter()
                    .zip(b_fields.iter())
                    .all(|((_, tx), (_, ty))| structural_type_eq(pool, *tx, *ty))
        }
        // Parametric tags: compare inner types recursively.
        // Without this, Option<int> would match Option<str> (same tag, different payload).
        Tag::Option => structural_type_eq(pool, pool.option_inner(a), pool.option_inner(b)),
        Tag::Result => {
            structural_type_eq(pool, pool.result_ok(a), pool.result_ok(b))
                && structural_type_eq(pool, pool.result_err(a), pool.result_err(b))
        }
        Tag::List => structural_type_eq(pool, pool.list_elem(a), pool.list_elem(b)),
        Tag::Set => structural_type_eq(pool, pool.set_elem(a), pool.set_elem(b)),
        Tag::Map => {
            structural_type_eq(pool, pool.map_key(a), pool.map_key(b))
                && structural_type_eq(pool, pool.map_value(a), pool.map_value(b))
        }
        Tag::Iterator | Tag::DoubleEndedIterator => {
            structural_type_eq(pool, pool.iterator_elem(a), pool.iterator_elem(b))
        }
        // Primitive/leaf tags (Int, Float, Bool, etc.): same tag + different
        // resolved Idx should not happen (singletons). If it does, reject the
        // match — the Idx equality check at the top already handles true equality.
        _ => false,
    }
}
