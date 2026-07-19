//! Pool-local variable and scheme identity remapping.

use rustc_hash::FxHashMap;

use crate::{GeneralizedVarState, Idx, Pool, Tag, UnboundVarState, VarState, DEFAULT_RANK};

use super::re_intern_type_with_var_remap;

/// Look up `src_var_id` in `var_remap` or allocate a fresh destination id.
///
/// Single SSOT for the "remap-or-allocate" pattern used by scheme binders,
/// leaf `Tag::Var` / `Tag::BoundVar` / `Tag::RigidVar`, and
/// `FunctionSig.scheme_var_ids` coherence. On first sighting of `src_var_id`,
/// allocates a fresh dst via [`Pool::allocate_var_id`], records the mapping in
/// `var_remap`, and rebuilds `target.var_states[dst_id]` variant-aware from
/// `source.var_states[src_var_id]` via [`rebuild_var_state`].
pub(super) fn get_or_allocate_var_id(
    src_var_id: u32,
    source: &Pool,
    target: &mut Pool,
    cache: &mut FxHashMap<Idx, Idx>,
    var_remap: &mut FxHashMap<u32, u32>,
) -> u32 {
    if let Some(&existing) = var_remap.get(&src_var_id) {
        return existing;
    }
    let new_id = target.allocate_var_id();
    var_remap.insert(src_var_id, new_id);
    rebuild_var_state(source, src_var_id, target, new_id, cache, var_remap);
    new_id
}

/// Re-intern a `Tag::Scheme` — remap binders FIRST so the body's leaf
/// `Tag::Var` references can resolve to the same destination ids through
/// `var_remap` during the recursive body walk.
///
/// A scheme whose body references a `var_id` not in its binder list (or vice
/// versa) is malformed; the scheme matrix cells (e2, e5) in
/// `pool/re_intern/tests.rs` pin this coherence invariant.
pub(super) fn re_intern_scheme(
    source: &Pool,
    idx: Idx,
    target: &mut Pool,
    cache: &mut FxHashMap<Idx, Idx>,
    var_remap: &mut FxHashMap<u32, u32>,
) -> Idx {
    let src_vars = source.scheme_vars(idx).to_vec();
    let mut dst_vars: Vec<u32> = Vec::with_capacity(src_vars.len());
    for &src_var_id in &src_vars {
        dst_vars.push(get_or_allocate_var_id(
            src_var_id, source, target, cache, var_remap,
        ));
    }
    let body =
        re_intern_type_with_var_remap(source, source.scheme_body(idx), target, cache, var_remap);
    target.scheme(&dst_vars, body)
}

/// Re-intern a leaf type-variable — `Tag::Var`, `Tag::BoundVar`, or
/// `Tag::RigidVar`. Remaps `data` (the pool-local `var_id`) to a
/// destination-local id via `var_remap`, allocating a fresh slot if this is
/// the first sighting of `src_var_id` in this re-intern session.
pub(super) fn re_intern_var_leaf(
    source: &Pool,
    idx: Idx,
    tag: Tag,
    target: &mut Pool,
    cache: &mut FxHashMap<Idx, Idx>,
    var_remap: &mut FxHashMap<u32, u32>,
) -> Idx {
    let src_var_id = source.data(idx);
    let dst_var_id = get_or_allocate_var_id(src_var_id, source, target, cache, var_remap);
    target.intern(tag, dst_var_id)
}

/// Rebuild `target.var_states[dst_var_id]` variant-aware from
/// `source.var_states[src_var_id]`.
///
/// Rebuild rules (scheme-coherence invariant SC-1):
/// - `Unbound { id, rank, name }` → `Unbound { id: dst_var_id, rank, name }`
///   (`id` is pool-local — must be the NEW destination id, not the source's).
/// - `Generalized { id, name }` → `Generalized { id: dst_var_id, name }`
///   (same pool-local id rule; preserves the `Generalized` variant so
///   `substitute_in_pool` takes the correct branch downstream).
/// - `Rigid { name }` → `Rigid { name }` (literal clone; `Name` is a global
///   intern, pool-independent).
/// - `Link { target }` → `Link { target: re_intern_type_with_var_remap(..) }`
///   (recursive re-intern of the link target; do NOT resolve via
///   `cache.get(&source.target).expect(..)` which panics when the link target
///   is reachable ONLY through this Link).
///
/// If the source has no `var_state` entry at `src_var_id` (e.g., a
/// test-fabricated `Tag::Var(7)` where the intern exists but no matching
/// `var_states` slot was registered), falls back to a default `Unbound` at
/// `dst_var_id` — the destination stands alone as a fresh unbound variable.
fn rebuild_var_state(
    source: &Pool,
    src_var_id: u32,
    target: &mut Pool,
    dst_var_id: u32,
    cache: &mut FxHashMap<Idx, Idx>,
    var_remap: &mut FxHashMap<u32, u32>,
) {
    // Clone source state to release the borrow on source before any target
    // mutation.
    let src_state = source.var_state_checked(src_var_id).cloned();

    let dst_state = match src_state {
        Some(VarState::Unbound(UnboundVarState { rank, name, .. })) => {
            VarState::Unbound(UnboundVarState {
                id: dst_var_id,
                rank,
                name,
            })
        }
        Some(VarState::Generalized(GeneralizedVarState { name, .. })) => {
            VarState::Generalized(GeneralizedVarState {
                id: dst_var_id,
                name,
            })
        }
        Some(VarState::Rigid { name }) => VarState::Rigid { name },
        Some(VarState::Link {
            target: src_link_target,
        }) => {
            // Recursive re-intern of the Link target. May mutate target,
            // cache, var_remap before we write the final dst_state.
            let dst_link_target =
                re_intern_type_with_var_remap(source, src_link_target, target, cache, var_remap);
            VarState::Link {
                target: dst_link_target,
            }
        }
        None => VarState::Unbound(UnboundVarState {
            id: dst_var_id,
            rank: DEFAULT_RANK,
            name: None,
        }),
    };

    // Defensive: if the caller allocated `dst_var_id` via `allocate_var_id`,
    // the slot already exists. If not (e.g., future callers that reserve via
    // `ensure_var_capacity`), extend capacity here.
    target.ensure_var_capacity(dst_var_id + 1);
    *target.var_state_mut(dst_var_id) = dst_state;
}
