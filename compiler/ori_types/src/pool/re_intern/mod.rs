//! Cross-pool type re-interning with var-id remap (append-only per `types.md §TY-6`).
//!
//! Re-interns types from a source `Pool` into a target `Pool`. The target
//! receives fresh entries via `target.intern(..)` / `target.scheme(..)` — the
//! source is never mutated, and existing target items are never rewritten.
//!
//! # Two API layers
//!
//! - [`re_intern_type`] / [`re_intern_sig`] — thin backward-compat wrappers.
//!   Each call allocates its own empty `var_remap`. Suitable for contexts that
//!   do not need to share var-id mappings across multiple re-intern calls.
//!
//! - [`re_intern_type_with_var_remap`] / [`re_intern_sig_with_var_remap`] —
//!   the var-remap-aware entry points. Required for cross-module pool-merge
//!   contexts (e.g., the test-runner at `compiler/oric/src/test/runner/llvm_backend.rs`)
//!   where the target pool's `var_states` was cloned from a different source
//!   than the imported types: imported `Tag::Var` / `Tag::BoundVar` /
//!   `Tag::RigidVar` / `Tag::Scheme` binder ids MUST be remapped to freshly
//!   allocated destination-local ids so they cannot alias host-module
//!   `var_states` slots. All three consumer sites for the same imported module
//!   SHARE a single `var_remap` map so sig binder ids and type-tree leaf ids
//!   stay coherent.
//!
//! # Cache vs `var_remap`
//!
//! Two independent maps thread through every re-intern call:
//!
//! - `cache: FxHashMap<Idx, Idx>` — `src_idx → dst_idx` for whole types
//!   already re-interned in this session. Prevents redundant work when the
//!   same type appears in multiple signatures.
//!
//! - `var_remap: FxHashMap<u32, u32>` — `src_var_id → dst_var_id` for every
//!   `var_id` encountered during re-intern. Shared across all re-intern calls
//!   from the same source module so `FunctionSig.scheme_var_ids` and the
//!   `Tag::Var` leaves they bind resolve to the SAME destination id.
//!
//! # Fast-path guards (step 7)
//!
//! The Merkle-hash fast path (`target.lookup_by_hash(source.hash(idx))`) is
//! skipped when either:
//!
//! - The source type has `HAS_VAR | HAS_BOUND_VAR | HAS_RIGID_VAR` flags set
//!   (pool-local `var_ids` are included in the leaf hash per `pool/mod.rs`
//!   leaf-hash; a fast-path hit would dedup distinct pool-local identities).
//! - The source tag is `Tag::Scheme` (the scheme hash is extra-backed per
//!   `types.md §TI-3` and includes binder ids, but `PROPAGATE_MASK` per
//!   `§TF-3` does not propagate `HAS_VAR` from a scheme's binder list to the
//!   parent scheme's flags — a flag-only guard would miss schemes with
//!   var-bearing binders and a var-free body, e.g. `Scheme([7], Tag::Int)`).

use rustc_hash::FxHashMap;

use crate::{FunctionSig, Idx, Pool, Tag, TypeFlags, VarState, DEFAULT_RANK};

// --- Public API ------------------------------------------------------------

/// Re-intern a single type from `source` pool into `target` pool.
///
/// Backward-compat wrapper: allocates a fresh empty `var_remap` and delegates
/// to [`re_intern_type_with_var_remap`]. For cross-pool-merge contexts that
/// need to share var-id mappings across multiple re-intern calls, use the
/// var-remap-aware variant directly.
#[allow(
    clippy::implicit_hasher,
    reason = "FxHashMap chosen for performance — generifying would defeat the purpose"
)]
pub fn re_intern_type(
    source: &Pool,
    idx: Idx,
    target: &mut Pool,
    cache: &mut FxHashMap<Idx, Idx>,
) -> Idx {
    let mut var_remap: FxHashMap<u32, u32> = FxHashMap::default();
    re_intern_type_with_var_remap(source, idx, target, cache, &mut var_remap)
}

/// Re-intern a type from `source` pool into `target` pool with per-import var-id remap.
///
/// For every imported `Tag::Var`, `Tag::BoundVar`, `Tag::RigidVar`, and `Tag::Scheme`
/// binder encountered, allocates a fresh destination `var_id` in `target` via
/// [`Pool::allocate_var_id`] and records `src_var_id → dst_var_id` in `var_remap`.
/// Destination `var_states[dst_var_id]` is rebuilt variant-aware from the source
/// state (preserving `Unbound.rank/name`, `Generalized.name`, `Rigid.name`, and
/// recursing through `Link.target`).
///
/// This prevents the cross-module pool-merge var-id collision documented at the
/// §08.1.R root cause: imported `var_ids` cannot alias `target.var_states` slots
/// belonging to the host module.
#[allow(
    clippy::implicit_hasher,
    reason = "FxHashMap chosen for performance — generifying would defeat the purpose"
)]
pub fn re_intern_type_with_var_remap(
    source: &Pool,
    idx: Idx,
    target: &mut Pool,
    cache: &mut FxHashMap<Idx, Idx>,
    var_remap: &mut FxHashMap<u32, u32>,
) -> Idx {
    // Session cache: same src_idx already re-interned.
    if let Some(&cached) = cache.get(&idx) {
        return cached;
    }

    let tag = source.tag(idx);

    // Primitives: fixed indices, identical across all pools.
    if tag.is_primitive() {
        cache.insert(idx, idx);
        return idx;
    }

    // Fast path: Merkle hash lookup. Guarded against var-bearing subtrees and
    // `Tag::Scheme` (see module docs §Fast-path guards).
    if fast_path_eligible(source, idx, tag) {
        let hash = source.hash(idx);
        if let Some(local_idx) = target.lookup_by_hash(hash) {
            cache.insert(idx, local_idx);
            return local_idx;
        }
    }

    // Slow path: recursively re-intern children, allocating fresh var_ids as
    // needed, then construct the parent in the target pool.
    let result = re_intern_by_tag(source, idx, tag, target, cache, var_remap);
    cache.insert(idx, result);
    result
}

/// Re-intern a [`FunctionSig`] from a source pool into a target pool.
///
/// Backward-compat wrapper: delegates to [`re_intern_sig_with_var_remap`] with
/// a fresh empty `var_remap`.
#[allow(
    clippy::implicit_hasher,
    reason = "FxHashMap chosen for performance — generifying would defeat the purpose"
)]
pub fn re_intern_sig(
    sig: &FunctionSig,
    source: &Pool,
    target: &mut Pool,
    cache: &mut FxHashMap<Idx, Idx>,
) -> FunctionSig {
    let mut var_remap: FxHashMap<u32, u32> = FxHashMap::default();
    re_intern_sig_with_var_remap(sig, source, target, cache, &mut var_remap)
}

/// Re-intern a [`FunctionSig`] with a shared `var_remap` map.
///
/// Rewrites `scheme_var_ids` through the same `var_remap` that rewrites leaf
/// `Tag::Var` ids encountered during `param_types` / `return_type` re-intern,
/// so the monomorphizer's `var_subst = HashMap::from([(scheme_var_ids[i],
/// concrete_arg[i])])` map at call sites resolves every leaf `Tag::Var` in
/// the remapped type tree.
///
/// If a `scheme_var_id` was not encountered during leaf re-intern (i.e., the
/// binder appears only in the sig metadata, not in any type position), a fresh
/// destination id is allocated for it too — this preserves the invariant that
/// every `scheme_var_id` is a valid `target`-local id after re-intern.
#[allow(
    clippy::implicit_hasher,
    reason = "FxHashMap chosen for performance — generifying would defeat the purpose"
)]
pub fn re_intern_sig_with_var_remap(
    sig: &FunctionSig,
    source: &Pool,
    target: &mut Pool,
    cache: &mut FxHashMap<Idx, Idx>,
    var_remap: &mut FxHashMap<u32, u32>,
) -> FunctionSig {
    let mut result = sig.clone();

    // Re-intern parameter types + return type. This populates `var_remap` for
    // every var_id encountered in the type tree.
    for param_type in &mut result.param_types {
        *param_type = re_intern_type_with_var_remap(source, *param_type, target, cache, var_remap);
    }
    result.return_type =
        re_intern_type_with_var_remap(source, result.return_type, target, cache, var_remap);

    // Rewrite scheme_var_ids through var_remap. Every binder in scheme_var_ids
    // MUST resolve to a destination-local id. If the binder wasn't encountered
    // during leaf re-intern (e.g., appears only in sig metadata, not in any
    // type position), allocate a fresh destination id for it too.
    for var_id in &mut result.scheme_var_ids {
        *var_id = get_or_allocate_var_id(*var_id, source, target, cache, var_remap);
    }

    // Recompute Merkle hashes in the target pool — param_types / return_type
    // may be different Idx values after remap.
    result.populate_hashes(target);
    result
}

// --- Internals --------------------------------------------------------------

/// Returns true iff the source type is eligible for the Merkle-hash fast path
/// in [`re_intern_type_with_var_remap`].
///
/// Var-bearing types and `Tag::Scheme` are excluded because their hashes
/// encode pool-local `var_ids` that would be incorrectly deduped across pools.
fn fast_path_eligible(source: &Pool, idx: Idx, tag: Tag) -> bool {
    if tag == Tag::Scheme {
        return false;
    }
    let flags = source.flags(idx);
    !flags.intersects(TypeFlags::HAS_VAR | TypeFlags::HAS_BOUND_VAR | TypeFlags::HAS_RIGID_VAR)
}

/// Tag-driven re-interning dispatch.
///
/// Every compound tag recursively re-interns its children through
/// [`re_intern_type_with_var_remap`] so `var_remap` is threaded consistently.
/// `Tag::Var | BoundVar | RigidVar` allocate fresh destination `var_ids` via
/// [`Pool::allocate_var_id`] and record the mapping in `var_remap`.
/// `Tag::Scheme` remaps binders BEFORE the body is re-interned (so body-leaf
/// `Tag::Var` references can find their binders in `var_remap`).
fn re_intern_by_tag(
    source: &Pool,
    idx: Idx,
    tag: Tag,
    target: &mut Pool,
    cache: &mut FxHashMap<Idx, Idx>,
    var_remap: &mut FxHashMap<u32, u32>,
) -> Idx {
    match tag {
        // Simple containers: data = child Idx
        Tag::List
        | Tag::Option
        | Tag::Set
        | Tag::Channel
        | Tag::Range
        | Tag::Iterator
        | Tag::DoubleEndedIterator => {
            let child = Idx::from_raw(source.data(idx));
            let local_child =
                re_intern_type_with_var_remap(source, child, target, cache, var_remap);
            target.intern(tag, local_child.raw())
        }

        // Two-child containers (Map, Result, Borrowed use extra array)
        Tag::Map => {
            let key = re_intern_type_with_var_remap(
                source,
                source.map_key(idx),
                target,
                cache,
                var_remap,
            );
            let val = re_intern_type_with_var_remap(
                source,
                source.map_value(idx),
                target,
                cache,
                var_remap,
            );
            target.map(key, val)
        }

        Tag::Result => {
            let ok = re_intern_type_with_var_remap(
                source,
                source.result_ok(idx),
                target,
                cache,
                var_remap,
            );
            let err = re_intern_type_with_var_remap(
                source,
                source.result_err(idx),
                target,
                cache,
                var_remap,
            );
            target.result(ok, err)
        }

        Tag::Borrowed => {
            let inner = re_intern_type_with_var_remap(
                source,
                source.borrowed_inner(idx),
                target,
                cache,
                var_remap,
            );
            let lifetime = source.borrowed_lifetime(idx);
            target.borrowed(inner, lifetime)
        }

        Tag::Function => re_intern_function(source, idx, target, cache, var_remap),
        Tag::Tuple => re_intern_tuple(source, idx, target, cache, var_remap),
        Tag::Struct => re_intern_struct(source, idx, target, cache, var_remap),
        Tag::Enum => re_intern_enum(source, idx, target, cache, var_remap),

        // Named: [name_lo, name_hi] — structural, no child types
        Tag::Named => {
            let name = source.named_name(idx);
            target.named(name)
        }

        Tag::Applied => re_intern_applied(source, idx, target, cache, var_remap),

        // Scheme: [var_count, v0_id, v1_id, ..., body_idx]
        Tag::Scheme => re_intern_scheme(source, idx, target, cache, var_remap),

        // Type variables: data = pool-local var_id.
        Tag::Var | Tag::BoundVar | Tag::RigidVar => {
            re_intern_var_leaf(source, idx, tag, target, cache, var_remap)
        }

        // Alias, Projection, ModuleNs, Infer, SelfType: transient/special types
        // that should not appear in codegen function signatures. The type checker
        // resolves these before output. If they somehow reach here, log a warning
        // and return the source idx as-is (will likely cause downstream errors
        // that surface the root cause).
        Tag::Alias | Tag::Projection | Tag::ModuleNs | Tag::Infer | Tag::SelfType => {
            tracing::warn!(
                ?tag,
                "re_intern_type: unexpected special type in codegen signature"
            );
            idx // Return as-is — these are errors if they reach codegen
        }

        _ => {
            tracing::warn!(?tag, "re_intern_type: unknown tag — returning source idx");
            idx
        }
    }
}

/// Re-intern a `Tag::Function` — `[param_count, p0, p1, ..., return_type]`.
fn re_intern_function(
    source: &Pool,
    idx: Idx,
    target: &mut Pool,
    cache: &mut FxHashMap<Idx, Idx>,
    var_remap: &mut FxHashMap<u32, u32>,
) -> Idx {
    let params: Vec<Idx> = source
        .function_params(idx)
        .into_iter()
        .map(|p| re_intern_type_with_var_remap(source, p, target, cache, var_remap))
        .collect();
    let ret = re_intern_type_with_var_remap(
        source,
        source.function_return(idx),
        target,
        cache,
        var_remap,
    );
    target.function(&params, ret)
}

/// Re-intern a `Tag::Tuple` — `[elem_count, e0, e1, ...]`.
fn re_intern_tuple(
    source: &Pool,
    idx: Idx,
    target: &mut Pool,
    cache: &mut FxHashMap<Idx, Idx>,
    var_remap: &mut FxHashMap<u32, u32>,
) -> Idx {
    let elems: Vec<Idx> = source
        .tuple_elems(idx)
        .into_iter()
        .map(|e| re_intern_type_with_var_remap(source, e, target, cache, var_remap))
        .collect();
    target.tuple(&elems)
}

/// Re-intern a `Tag::Applied` — `[name_lo, name_hi, arg_count, a0, a1, ...]`.
fn re_intern_applied(
    source: &Pool,
    idx: Idx,
    target: &mut Pool,
    cache: &mut FxHashMap<Idx, Idx>,
    var_remap: &mut FxHashMap<u32, u32>,
) -> Idx {
    let name = source.applied_name(idx);
    let args: Vec<Idx> = source
        .applied_args(idx)
        .into_iter()
        .map(|a| re_intern_type_with_var_remap(source, a, target, cache, var_remap))
        .collect();
    target.applied(name, &args)
}

/// Re-intern a struct type (name + typed fields).
fn re_intern_struct(
    source: &Pool,
    idx: Idx,
    target: &mut Pool,
    cache: &mut FxHashMap<Idx, Idx>,
    var_remap: &mut FxHashMap<u32, u32>,
) -> Idx {
    let name = source.struct_name(idx);
    let fields: Vec<(ori_ir::Name, Idx)> = source
        .struct_fields(idx)
        .into_iter()
        .map(|(fname, ftype)| {
            (
                fname,
                re_intern_type_with_var_remap(source, ftype, target, cache, var_remap),
            )
        })
        .collect();
    target.struct_type(name, &fields)
}

/// Re-intern an enum type (name + variants with typed fields).
fn re_intern_enum(
    source: &Pool,
    idx: Idx,
    target: &mut Pool,
    cache: &mut FxHashMap<Idx, Idx>,
    var_remap: &mut FxHashMap<u32, u32>,
) -> Idx {
    let name = source.enum_name(idx);
    let variants: Vec<crate::pool::construct::EnumVariant> = source
        .enum_variants(idx)
        .into_iter()
        .map(|(vname, vtypes)| {
            let re_interned_fields: Vec<Idx> = vtypes
                .into_iter()
                .map(|t| re_intern_type_with_var_remap(source, t, target, cache, var_remap))
                .collect();
            crate::pool::construct::EnumVariant {
                name: vname,
                field_types: re_interned_fields,
            }
        })
        .collect();
    target.enum_type(name, &variants)
}

/// Look up `src_var_id` in `var_remap` or allocate a fresh destination id.
///
/// Single SSOT for the "remap-or-allocate" pattern used by scheme binders,
/// leaf `Tag::Var` / `Tag::BoundVar` / `Tag::RigidVar`, and
/// `FunctionSig.scheme_var_ids` coherence. On first sighting of `src_var_id`,
/// allocates a fresh dst via [`Pool::allocate_var_id`], records the mapping in
/// `var_remap`, and rebuilds `target.var_states[dst_id]` variant-aware from
/// `source.var_states[src_var_id]` via [`rebuild_var_state`].
fn get_or_allocate_var_id(
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
/// versa) is malformed; the plan's §08.2 cells (e2, e5) pin this coherence
/// invariant.
fn re_intern_scheme(
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
fn re_intern_var_leaf(
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
/// Per the plan §08.1.5 step 6 and `types.md §SC-1`:
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
        Some(VarState::Unbound { rank, name, .. }) => VarState::Unbound {
            id: dst_var_id,
            rank,
            name,
        },
        Some(VarState::Generalized { name, .. }) => VarState::Generalized {
            id: dst_var_id,
            name,
        },
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
        None => VarState::Unbound {
            id: dst_var_id,
            rank: DEFAULT_RANK,
            name: None,
        },
    };

    // Defensive: if the caller allocated `dst_var_id` via `allocate_var_id`,
    // the slot already exists. If not (e.g., future callers that reserve via
    // `ensure_var_capacity`), extend capacity here.
    target.ensure_var_capacity(dst_var_id + 1);
    *target.var_state_mut(dst_var_id) = dst_state;
}

#[cfg(test)]
mod tests;
