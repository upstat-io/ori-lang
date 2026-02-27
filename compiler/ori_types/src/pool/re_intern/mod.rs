//! Cross-pool type re-interning using Merkle hashes.
//!
//! Re-interns types from a source Pool into a target Pool. Uses Merkle hash
//! lookup for O(1) fast path (the type already exists in the target), with
//! recursive structural walk as fallback (the type is new to the target).
//!
//! This enables single-pool codegen: imported modules' types get re-interned
//! into the main compilation pool before any downstream pass (ARC lowering,
//! LLVM codegen) runs, eliminating cross-pool Idx misuse.
//!
//! # Design
//!
//! Follows the same pattern as Git's content-addressed object model:
//! - If the hash exists in the target → O(1) lookup, done
//! - If not → recursively reconstruct from structure (children first)
//!
//! A per-call `FxHashMap<Idx, Idx>` cache ensures each unique source Idx
//! is re-interned at most once, even when the same type appears in multiple
//! function signatures or expression types.

use rustc_hash::FxHashMap;

use crate::{FunctionSig, Idx, Pool, Tag};

/// Re-intern a single type from `source` pool into `target` pool.
///
/// Uses Merkle hash for O(1) fast path. Falls back to recursive structural
/// walk for types not yet present in the target pool.
///
/// The `cache` maps source Idx → target Idx for types already processed in
/// this re-interning session. Callers should reuse the same cache across
/// multiple calls (e.g., all parameters of a function signature) to avoid
/// redundant work.
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
    // Check session cache first
    if let Some(&cached) = cache.get(&idx) {
        return cached;
    }

    // Primitives have fixed indices — identical across all pools
    let tag = source.tag(idx);
    if tag.is_primitive() {
        cache.insert(idx, idx);
        return idx;
    }

    // Fast path: Merkle hash lookup
    let hash = source.hash(idx);
    if let Some(local_idx) = target.lookup_by_hash(hash) {
        cache.insert(idx, local_idx);
        return local_idx;
    }

    // Slow path: recursively re-intern children, then construct in target
    let result = re_intern_by_tag(source, idx, tag, target, cache);
    cache.insert(idx, result);
    result
}

/// Re-intern a `FunctionSig` from a source pool into a target pool.
///
/// Returns an owned `FunctionSig` with all `Idx` values valid in `target`.
/// The `cache` is reused across multiple signatures from the same source
/// module to avoid redundant re-interning.
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
    let mut result = sig.clone();

    // Re-intern parameter types
    for param_type in &mut result.param_types {
        *param_type = re_intern_type(source, *param_type, target, cache);
    }

    // Re-intern return type
    result.return_type = re_intern_type(source, result.return_type, target, cache);

    // Recompute hashes from the target pool
    result.populate_hashes(target);

    result
}

/// Tag-driven re-interning dispatch.
///
/// Handles each type category by recursively re-interning children,
/// then constructing the parent type in the target pool.
fn re_intern_by_tag(
    source: &Pool,
    idx: Idx,
    tag: Tag,
    target: &mut Pool,
    cache: &mut FxHashMap<Idx, Idx>,
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
            let local_child = re_intern_type(source, child, target, cache);
            target.intern(tag, local_child.raw())
        }

        // Two-child containers (Map, Result, Borrowed use extra array)
        Tag::Map => {
            let key = re_intern_type(source, source.map_key(idx), target, cache);
            let val = re_intern_type(source, source.map_value(idx), target, cache);
            target.map(key, val)
        }

        Tag::Result => {
            let ok = re_intern_type(source, source.result_ok(idx), target, cache);
            let err = re_intern_type(source, source.result_err(idx), target, cache);
            target.result(ok, err)
        }

        Tag::Borrowed => {
            let inner = re_intern_type(source, source.borrowed_inner(idx), target, cache);
            let lifetime = source.borrowed_lifetime(idx);
            target.borrowed(inner, lifetime)
        }

        // Function: [param_count, p0, p1, ..., return_type]
        Tag::Function => {
            let params: Vec<Idx> = source
                .function_params(idx)
                .into_iter()
                .map(|p| re_intern_type(source, p, target, cache))
                .collect();
            let ret = re_intern_type(source, source.function_return(idx), target, cache);
            target.function(&params, ret)
        }

        // Tuple: [elem_count, e0, e1, ...]
        Tag::Tuple => {
            let elems: Vec<Idx> = source
                .tuple_elems(idx)
                .into_iter()
                .map(|e| re_intern_type(source, e, target, cache))
                .collect();
            target.tuple(&elems)
        }

        Tag::Struct => re_intern_struct(source, idx, target, cache),
        Tag::Enum => re_intern_enum(source, idx, target, cache),

        // Named: [name_lo, name_hi] — structural, no child types
        Tag::Named => {
            let name = source.named_name(idx);
            target.named(name)
        }

        // Applied: [name_lo, name_hi, arg_count, a0, a1, ...]
        Tag::Applied => {
            let name = source.applied_name(idx);
            let args: Vec<Idx> = source
                .applied_args(idx)
                .into_iter()
                .map(|a| re_intern_type(source, a, target, cache))
                .collect();
            target.applied(name, &args)
        }

        // Scheme: [var_count, v0_id, v1_id, ..., body_idx]
        Tag::Scheme => {
            let vars = source.scheme_vars(idx).to_vec();
            let body = re_intern_type(source, source.scheme_body(idx), target, cache);
            target.scheme(&vars, body)
        }

        // Type variables: data = var_id (pool-independent)
        Tag::Var | Tag::BoundVar | Tag::RigidVar => target.intern(tag, source.data(idx)),

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

/// Re-intern a struct type (name + typed fields).
fn re_intern_struct(
    source: &Pool,
    idx: Idx,
    target: &mut Pool,
    cache: &mut FxHashMap<Idx, Idx>,
) -> Idx {
    let name = source.struct_name(idx);
    let fields: Vec<(ori_ir::Name, Idx)> = source
        .struct_fields(idx)
        .into_iter()
        .map(|(fname, ftype)| (fname, re_intern_type(source, ftype, target, cache)))
        .collect();
    target.struct_type(name, &fields)
}

/// Re-intern an enum type (name + variants with typed fields).
fn re_intern_enum(
    source: &Pool,
    idx: Idx,
    target: &mut Pool,
    cache: &mut FxHashMap<Idx, Idx>,
) -> Idx {
    let name = source.enum_name(idx);
    let variants: Vec<crate::pool::construct::EnumVariant> = source
        .enum_variants(idx)
        .into_iter()
        .map(|(vname, vtypes)| {
            let re_interned_fields: Vec<Idx> = vtypes
                .into_iter()
                .map(|t| re_intern_type(source, t, target, cache))
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
