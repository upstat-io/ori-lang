//! Canonical representation mapping.
//!
//! Maps every `Tag` variant to its canonical `MachineRepr` — the
//! representation before any optimization. This is the starting point
//! for the `ReprPlan`: every type gets its canonical repr first,
//! then optimization passes narrow it.

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
/// calling [`canonical_cached()`] for each. Skips error types and the reserved
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
            // ERROR is a sentinel type — not a real type that reaches codegen.
            // Canonicalize as Unit (zero-size, trivial) so ReprPlan::is_trivial()
            // returns true, matching classify_triviality() and ArcClassifier.
            // Without this, is_trivial(ERROR) falls through to None→false,
            // creating triviality drift (TPR-02-005).
            plan.set_repr(
                idx,
                ReprDecision {
                    source: DecisionSource::Canonical,
                    type_idx: idx,
                    repr: MachineRepr::Unit,
                    reason: DecisionReason::Canonical,
                },
            );
            continue;
        }
        // Primitives always have a canonical representation.
        let Some(repr) = canonical_cached(pool, idx, &mut cache) else {
            // Should never happen — primitives are always canonicalizeable.
            tracing::error!(?idx, "primitive type has no canonical representation");
            continue;
        };
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

        // canonical_cached() returns None for types whose children contain
        // unresolvable type-checker artifacts (Error, Borrowed, Var inside
        // generic struct fields, etc.). These types won't reach codegen —
        // the TypeInfoStore fallback handles them.
        if let Some(repr) = canonical_cached(pool, idx, &mut cache) {
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

/// Compute the canonical machine representation for a type.
///
/// Returns `None` for types that cannot be canonicalized (unresolved type
/// variables, error types, internal-only types). This is the expected path
/// for type-checker artifacts that won't reach codegen.
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
/// Panics if the type returns `None` — this indicates a test bug
/// (invalid type passed to a test that expects valid input).
#[cfg(test)]
pub(crate) fn canonical(pool: &Pool, idx: Idx) -> MachineRepr {
    let Some(repr) = canonical_cached(pool, idx, &mut FxHashMap::default()) else {
        panic!("canonical: test input at {idx:?} has no valid representation");
    };
    repr
}

/// Compute the canonical representation with a shared memoization cache.
///
/// Returns `None` for types that cannot be canonicalized (unresolved
/// variables, error types, internal-only types, or composite types whose
/// children contain such types).
///
/// The cache ensures that mutually recursive types (A→B→A) produce the
/// same `MachineRepr` for each `Idx` regardless of traversal order
/// (TPR-01-021). Once a type is computed, it is cached and returned
/// for all future lookups.
pub(crate) fn canonical_cached(
    pool: &Pool,
    idx: Idx,
    cache: &mut FxHashMap<Idx, MachineRepr>,
) -> Option<MachineRepr> {
    canonical_inner(pool, idx, &mut FxHashSet::default(), cache)
}

/// Inner canonicalization with cycle detection via `visiting` set and
/// cross-call memoization via `cache`.
///
/// Returns `None` for types that cannot be canonicalized: unresolved
/// type variables, error types, internal-only types (Scheme, Projection,
/// etc.), and unresolved Named/Applied/Alias. Composite types whose
/// children return `None` also return `None` (fallibility propagates).
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
) -> Option<MachineRepr> {
    let resolved = pool.resolve_fully(idx);

    // Cache hit — return previously computed representation.
    if let Some(repr) = cache.get(&resolved) {
        return Some(repr.clone());
    }

    // Cycle detection: if already canonicalizing this type, it's a recursive
    // reference — return an RC pointer (recursive fields are heap-allocated).
    if !visiting.insert(resolved) {
        return Some(MachineRepr::RcPointer(RcRepr {
            rc_width: IntWidth::I64,
            atomic: true,
            inner: Box::new(MachineRepr::OpaquePtr),
            stack_promotable: false,
        }));
    }

    let tag = pool.tag(resolved);

    let result = match tag {
        // Primitives
        Tag::Int => Some(MachineRepr::Int {
            width: IntWidth::I64,
            signed: true,
        }),
        Tag::Float => Some(MachineRepr::Float {
            width: FloatWidth::F64,
        }),
        Tag::Bool => Some(MachineRepr::Bool),
        Tag::Char => Some(MachineRepr::Char),
        Tag::Byte => Some(MachineRepr::Byte),
        Tag::Duration => Some(MachineRepr::Duration),
        Tag::Size => Some(MachineRepr::Size),
        Tag::Ordering => Some(MachineRepr::Ordering),
        Tag::Unit => Some(MachineRepr::Unit),
        Tag::Never => Some(MachineRepr::Never),
        Tag::Str => Some(MachineRepr::FatPointer(FatRepr::Str)),
        Tag::Range => Some(MachineRepr::Range),
        Tag::Iterator | Tag::DoubleEndedIterator => Some(MachineRepr::UnmanagedPtr),
        Tag::Channel => Some(MachineRepr::OpaquePtr),

        // Collections — fat pointer {len, cap, data}
        Tag::List => canonical_collection(pool, pool.list_elem(resolved), visiting, cache),
        Tag::Set => canonical_collection(pool, pool.set_elem(resolved), visiting, cache),
        Tag::Map => canonical_map(pool, resolved, visiting, cache),

        // Composite types
        Tag::Option => {
            let inner = canonical_inner(pool, pool.option_inner(resolved), visiting, cache)?;
            Some(canonical_option(&inner))
        }
        Tag::Result => {
            let ok = canonical_inner(pool, pool.result_ok(resolved), visiting, cache)?;
            let err = canonical_inner(pool, pool.result_err(resolved), visiting, cache)?;
            Some(canonical_result(&ok, &err))
        }
        Tag::Function => canonical_function(pool, resolved, visiting, cache),
        Tag::Tuple => canonical_tuple(pool, resolved, visiting, cache),
        Tag::Struct => canonical_struct(pool, resolved, visiting, cache),
        Tag::Enum => canonical_enum(pool, resolved, visiting, cache),

        // Types that cannot be canonicalized — return None.
        // These are type-checker artifacts or error types that should not
        // reach codegen. The caller (populate_canonical) skips them.
        Tag::Named
        | Tag::Applied
        | Tag::Alias
        | Tag::Borrowed
        | Tag::Error
        | Tag::Var
        | Tag::BoundVar
        | Tag::RigidVar
        | Tag::Scheme
        | Tag::Projection
        | Tag::ModuleNs
        | Tag::Infer
        | Tag::SelfType => {
            // Clean up visiting set before returning None.
            visiting.remove(&resolved);
            return None;
        }
    };

    visiting.remove(&resolved);
    // Cache the result for cross-call consistency (TPR-01-021).
    if let Some(ref repr) = result {
        cache.insert(resolved, repr.clone());
    }
    result
}
