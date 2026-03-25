//! Canonical representation mapping.
//!
//! Maps every `Tag` variant to its canonical `MachineRepr` — the
//! representation before any optimization. This is the starting point
//! for the `ReprPlan`: every type gets its canonical repr first,
//! then §02–§11 narrow it.

mod type_repr;

use ori_types::{Idx, Pool, Tag};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::plan::{DecisionReason, DecisionSource, ReprDecision, ReprPlan};
use crate::repr::{FloatWidth, IntWidth, MachineRepr};
use crate::struct_repr::{FatRepr, RcRepr};

use type_repr::{
    canonical_collection, canonical_enum, canonical_function, canonical_map, canonical_option,
    canonical_result, canonical_struct, canonical_tuple,
};

/// Populate the `ReprPlan` with canonical representations for all types in the pool.
///
/// Iterates over the 12 primitive indices and all dynamically allocated types,
/// calling [`canonical()`] for each. Skips error types and the reserved
/// primitive range (12–63). Each canonical decision is recorded in the plan
/// with `DecisionSource::Canonical`.
pub(crate) fn populate_canonical(plan: &mut ReprPlan, pool: &Pool) {
    let pool_len = u32::try_from(pool.len()).unwrap_or(u32::MAX);

    // Shared memoization cache — persists across all canonical() calls so that
    // mutually recursive types (A→B→A) get the same MachineRepr regardless of
    // which root was canonicalized first (TPR-01-021).
    let mut cache = FxHashMap::default();

    // Canonicalize primitives (0–11).
    for raw in 0..Idx::PRIMITIVE_COUNT {
        let idx = Idx::from_raw(raw);
        if idx == Idx::ERROR {
            continue;
        }
        let repr = canonical_cached(pool, idx, &mut cache);
        plan.set_repr(
            idx,
            ReprDecision {
                source: DecisionSource::Canonical,
                type_idx: idx,
                repr,
                reason: DecisionReason::Canonical,
            },
        );
    }

    // Canonicalize dynamic types (FIRST_DYNAMIC..pool_len).
    let mut populated: u32 = 0;
    let mut skipped: u32 = 0;
    for raw in Idx::FIRST_DYNAMIC..pool_len {
        let idx = Idx::from_raw(raw);
        let tag = pool.tag(idx);

        // Skip unresolved / internal types that should not reach codegen.
        if matches!(
            tag,
            Tag::Var
                | Tag::BoundVar
                | Tag::RigidVar
                | Tag::Scheme
                | Tag::Projection
                | Tag::ModuleNs
                | Tag::Infer
                | Tag::SelfType
        ) {
            continue;
        }

        // Skip types that contain unresolved type variables (e.g., generic
        // function signatures like `(T) -> T`). These are type-checker
        // artifacts that won't reach codegen — only monomorphized types do.
        let flags = pool.flags(idx);
        if flags.has_vars() {
            continue;
        }

        // Skip types that resolve_fully() can't fully resolve (Named/Applied/
        // Alias pointing to unregistered or circular types). These are
        // type-checker artifacts that won't reach codegen.
        let resolved = pool.resolve_fully(idx);
        let resolved_tag = pool.tag(resolved);
        if matches!(
            resolved_tag,
            Tag::Named
                | Tag::Applied
                | Tag::Alias
                | Tag::Var
                | Tag::BoundVar
                | Tag::RigidVar
                | Tag::Scheme
                | Tag::Projection
                | Tag::ModuleNs
                | Tag::Infer
                | Tag::SelfType
                | Tag::Borrowed
                | Tag::Error
        ) {
            continue;
        }

        // Use try_canonical_cached() to gracefully handle composite types whose
        // children contain unresolvable type-checker artifacts (Error, Borrowed,
        // Var inside generic struct fields, etc.). These types won't reach
        // codegen — the TypeInfoStore fallback handles them.
        if let Some(repr) = try_canonical_cached(pool, idx, &mut cache) {
            plan.set_repr(
                idx,
                ReprDecision {
                    source: DecisionSource::Canonical,
                    type_idx: idx,
                    repr,
                    reason: DecisionReason::Canonical,
                },
            );
            populated += 1;
        } else {
            skipped += 1;
        }
    }

    tracing::debug!(
        primitives = Idx::PRIMITIVE_COUNT - 1,
        populated,
        skipped,
        "populated canonical representations"
    );
}

/// Try to compute the canonical representation with a shared cache, returning
/// `None` if the type contains unresolvable children (type variables, etc.).
///
/// Used by [`populate_canonical()`] which iterates ALL pool types, including
/// type-checker artifacts that `canonical()` would panic on.
fn try_canonical_cached(
    pool: &Pool,
    idx: Idx,
    cache: &mut FxHashMap<Idx, MachineRepr>,
) -> Option<MachineRepr> {
    // Clone the cache before catch_unwind so a panicking computation doesn't
    // leave partial entries. The cache is cheap to clone (small during §01).
    let snapshot = cache.clone();
    if let Ok(repr) = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        canonical_cached(pool, idx, cache)
    })) {
        Some(repr)
    } else {
        // Restore cache to pre-panic state.
        *cache = snapshot;
        None
    }
}

/// Compute the canonical machine representation for a type.
///
/// Handles recursive types (e.g., `type Tree = Leaf(int) | Node(Tree, Tree)`)
/// via cycle detection — recursive positions are represented as
/// [`MachineRepr::RcPointer`] since they are always heap-allocated in Ori.
///
/// # Visibility
///
/// This function is test-only because standalone calls without a shared
/// cache do NOT guarantee SCC consistency for mutually recursive types.
/// The production contract is [`compute_repr_plan()`](crate::compute_repr_plan),
/// which calls [`populate_canonical()`] with a shared cache to ensure each
/// `Idx` gets one stable representation regardless of traversal order.
///
/// # Panics
///
/// Panics if an unresolved type variable (`Var`, `BoundVar`, `RigidVar`,
/// `Scheme`, `Projection`, `ModuleNs`, `Infer`, `SelfType`) reaches
/// this function — these indicate a type checker bug.
#[cfg(test)]
pub(crate) fn canonical(pool: &Pool, idx: Idx) -> MachineRepr {
    canonical_cached(pool, idx, &mut FxHashMap::default())
}

/// Compute the canonical representation with a shared memoization cache.
///
/// The cache ensures that mutually recursive types (A→B→A) produce the
/// same `MachineRepr` for each `Idx` regardless of traversal order
/// (TPR-01-021). Once a type is computed, it is cached and returned
/// for all future lookups.
pub(crate) fn canonical_cached(
    pool: &Pool,
    idx: Idx,
    cache: &mut FxHashMap<Idx, MachineRepr>,
) -> MachineRepr {
    canonical_inner(pool, idx, &mut FxHashSet::default(), cache)
}

/// Inner canonicalization with cycle detection via `visiting` set and
/// cross-call memoization via `cache`.
///
/// When a type is encountered that is already being canonicalized
/// (i.e., present in `visiting`), we return an `RcPointer` — recursive
/// positions in Ori are always behind ARC pointers at runtime.
///
/// The `cache` persists across calls in `populate_canonical()` so that
/// mutually recursive types get consistent representations (TPR-01-021).
fn canonical_inner(
    pool: &Pool,
    idx: Idx,
    visiting: &mut FxHashSet<Idx>,
    cache: &mut FxHashMap<Idx, MachineRepr>,
) -> MachineRepr {
    let resolved = pool.resolve_fully(idx);

    // Cache hit — return previously computed representation.
    if let Some(repr) = cache.get(&resolved) {
        return repr.clone();
    }

    // Cycle detection: if already canonicalizing this type, it's a recursive
    // reference — return an RC pointer (recursive fields are heap-allocated).
    if !visiting.insert(resolved) {
        return MachineRepr::RcPointer(RcRepr {
            rc_width: IntWidth::I64,
            atomic: true,
            inner: Box::new(MachineRepr::OpaquePtr),
            stack_promotable: false,
        });
    }

    let tag = pool.tag(resolved);

    let result = match tag {
        // Primitives
        Tag::Int => MachineRepr::Int {
            width: IntWidth::I64,
            signed: true,
        },
        Tag::Float => MachineRepr::Float {
            width: FloatWidth::F64,
        },
        Tag::Bool => MachineRepr::Bool,
        Tag::Char => MachineRepr::Char,
        Tag::Byte => MachineRepr::Byte,
        Tag::Duration => MachineRepr::Duration,
        Tag::Size => MachineRepr::Size,
        Tag::Ordering => MachineRepr::Ordering,
        Tag::Unit => MachineRepr::Unit,
        Tag::Never => MachineRepr::Never,
        Tag::Str => MachineRepr::FatPointer(FatRepr::Str),
        Tag::Range => MachineRepr::Range,
        Tag::Iterator | Tag::DoubleEndedIterator | Tag::Channel => MachineRepr::OpaquePtr,

        // Collections — fat pointer {len, cap, data}
        Tag::List => canonical_collection(pool, pool.list_elem(resolved), visiting, cache),
        Tag::Set => canonical_collection(pool, pool.set_elem(resolved), visiting, cache),
        Tag::Map => canonical_map(pool, resolved, visiting, cache),

        // Composite types
        Tag::Option => canonical_option(canonical_inner(
            pool,
            pool.option_inner(resolved),
            visiting,
            cache,
        )),
        Tag::Result => {
            let ok = canonical_inner(pool, pool.result_ok(resolved), visiting, cache);
            let err = canonical_inner(pool, pool.result_err(resolved), visiting, cache);
            canonical_result(ok, err)
        }
        Tag::Function => canonical_function(pool, resolved, visiting, cache),
        Tag::Tuple => canonical_tuple(pool, resolved, visiting, cache),
        Tag::Struct => canonical_struct(pool, resolved, visiting, cache),
        Tag::Enum => canonical_enum(pool, resolved, visiting, cache),

        // Types that must not reach canonical — compiler bugs
        Tag::Named | Tag::Applied | Tag::Alias => panic!(
            "canonical: Named/Applied/Alias should be resolved by resolve_fully, \
             got {tag:?} at idx {resolved:?}"
        ),
        Tag::Borrowed | Tag::Error => {
            panic!("canonical: {tag:?} at idx {resolved:?} should not reach codegen")
        }
        Tag::Var | Tag::BoundVar | Tag::RigidVar => panic!(
            "canonical: unresolved type variable {tag:?} at idx {resolved:?} — \
             all variables must be resolved before codegen"
        ),
        Tag::Scheme | Tag::Projection | Tag::ModuleNs | Tag::Infer | Tag::SelfType => {
            panic!("canonical: special type {tag:?} at idx {resolved:?} should never reach codegen")
        }
    };

    visiting.remove(&resolved);
    // Cache the result for cross-call consistency (TPR-01-021).
    cache.insert(resolved, result.clone());
    result
}
