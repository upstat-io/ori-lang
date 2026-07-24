//! Cross-pool type re-interning with var-id remap.
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
//! - The source tag is `Tag::Scheme` (TF-3).

use rustc_hash::FxHashMap;

mod variables;

use variables::{get_or_allocate_var_id, re_intern_scheme, re_intern_var_leaf};

use crate::{FunctionSig, Idx, Pool, Tag, TypeFlags};

// Public API

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
/// This prevents the cross-module pool-merge var-id collision:
/// imported `var_ids` cannot alias `target.var_states` slots
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
            carry_nominal_resolution(source, idx, tag, target, local_idx, cache, var_remap);
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
    for capability in &mut result.capability_params {
        let crate::CapabilityParam::Value {
            provider_type,
            provider_var_id,
            ..
        } = capability
        else {
            continue;
        };
        *provider_type =
            re_intern_type_with_var_remap(source, *provider_type, target, cache, var_remap);
        *provider_var_id =
            get_or_allocate_var_id(*provider_var_id, source, target, cache, var_remap);
    }

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

// Internals

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
            let new_named = target.named(name);
            // Publish the wrapper before rebuilding its concrete body. A
            // recursive body can refer back to this same Named identity.
            cache.insert(idx, new_named);
            carry_nominal_resolution(source, idx, Tag::Named, target, new_named, cache, var_remap);
            // Carry the FFI C-ABI kind side table (keyed by the Named Idx) — re-
            // interning the Named structure alone drops it.
            if let Some(kind) = source.cabi_kind(idx) {
                target.set_cabi_kind(new_named, kind);
            }
            new_named
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
    let target_applied = target.applied(name, &args);
    // Publish the wrapper before rebuilding its concrete body so recursive
    // generic composites terminate through the session cache.
    cache.insert(idx, target_applied);
    // A materialized generic-composite `Applied` carries its concrete
    // `Struct`/`Enum` body in the source pool's `resolutions` map. Re-interning the
    // `Applied` alone drops that resolution, so the AOT codegen pool (which migrates
    // into a fresh pool, unlike the JIT path) loses the concrete layout and emits a
    // generic-placeholder struct + malformed `ori_rc_dec`. Carry the resolution.
    carry_nominal_resolution(
        source,
        idx,
        Tag::Applied,
        target,
        target_applied,
        cache,
        var_remap,
    );
    target_applied
}

/// Rebuild the concrete body paired with a Named/Applied wrapper.
///
/// Pool resolutions are part of a nominal type's usable identity: ARC and
/// representation consumers resolve wrappers to their concrete fields and
/// variants. The source wrapper must already be present in `cache` before this
/// helper runs so recursive definitions terminate at the target wrapper.
fn carry_nominal_resolution(
    source: &Pool,
    source_idx: Idx,
    source_tag: Tag,
    target: &mut Pool,
    target_idx: Idx,
    cache: &mut FxHashMap<Idx, Idx>,
    var_remap: &mut FxHashMap<u32, u32>,
) {
    if !matches!(source_tag, Tag::Named | Tag::Applied) {
        return;
    }
    let Some(concrete) = source.resolve(source_idx) else {
        return;
    };
    let target_concrete = re_intern_type_with_var_remap(source, concrete, target, cache, var_remap);
    target.set_resolution(target_idx, target_concrete);
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

#[cfg(test)]
mod tests;
