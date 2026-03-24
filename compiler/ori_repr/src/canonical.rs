//! Canonical representation mapping.
//!
//! Maps every `Tag` variant to its canonical `MachineRepr` — the
//! representation before any optimization. This is the starting point
//! for the `ReprPlan`: every type gets its canonical repr first,
//! then §02–§11 narrow it.

use ori_ir::Name;
use ori_types::{Idx, Pool, Tag};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::enum_repr::{EnumRepr, EnumTag, VariantRepr};
use crate::layout::{
    compute_field_layout, compute_payload_layout, field_align, field_size, is_trivial_repr,
    round_up,
};
use crate::plan::{DecisionReason, DecisionSource, ReprDecision, ReprPlan};
use crate::repr::{FloatWidth, IntWidth, MachineRepr};
use crate::struct_repr::{ClosureRepr, FatRepr, FieldRepr, RcRepr, StructRepr, TupleRepr};

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
/// # Panics
///
/// Panics if an unresolved type variable (`Var`, `BoundVar`, `RigidVar`,
/// `Scheme`, `Projection`, `ModuleNs`, `Infer`, `SelfType`) reaches
/// this function — these indicate a type checker bug.
pub fn canonical(pool: &Pool, idx: Idx) -> MachineRepr {
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

/// Canonicalize a collection element into a fat pointer.
fn canonical_collection(
    pool: &Pool,
    elem_idx: Idx,
    visiting: &mut FxHashSet<Idx>,
    cache: &mut FxHashMap<Idx, MachineRepr>,
) -> MachineRepr {
    MachineRepr::FatPointer(FatRepr::Collection {
        element_repr: Box::new(canonical_inner(pool, elem_idx, visiting, cache)),
    })
}

/// Canonicalize a map into a fat pointer with key and value reprs.
fn canonical_map(
    pool: &Pool,
    resolved: Idx,
    visiting: &mut FxHashSet<Idx>,
    cache: &mut FxHashMap<Idx, MachineRepr>,
) -> MachineRepr {
    MachineRepr::FatPointer(FatRepr::Map {
        key_repr: Box::new(canonical_inner(
            pool,
            pool.map_key(resolved),
            visiting,
            cache,
        )),
        value_repr: Box::new(canonical_inner(
            pool,
            pool.map_value(resolved),
            visiting,
            cache,
        )),
    })
}

/// Canonicalize a function type into a closure representation.
fn canonical_function(
    pool: &Pool,
    resolved: Idx,
    visiting: &mut FxHashSet<Idx>,
    cache: &mut FxHashMap<Idx, MachineRepr>,
) -> MachineRepr {
    let params: Vec<MachineRepr> = pool
        .function_params(resolved)
        .into_iter()
        .map(|p| canonical_inner(pool, p, visiting, cache))
        .collect();
    let ret = canonical_inner(pool, pool.function_return(resolved), visiting, cache);
    MachineRepr::Closure(ClosureRepr {
        params,
        ret: Box::new(ret),
    })
}

/// Canonicalize a tuple into an anonymous struct with positional fields.
fn canonical_tuple(
    pool: &Pool,
    resolved: Idx,
    visiting: &mut FxHashSet<Idx>,
    cache: &mut FxHashMap<Idx, MachineRepr>,
) -> MachineRepr {
    let fields: Vec<FieldRepr> = pool
        .tuple_elems(resolved)
        .into_iter()
        .enumerate()
        .map(|(i, elem_idx)| {
            let repr = canonical_inner(pool, elem_idx, visiting, cache);
            let idx_u32 = u32::try_from(i).unwrap_or(u32::MAX);
            FieldRepr {
                name: Name::new(0, idx_u32),
                original_index: idx_u32,
                offset: 0, // Set by §06 layout
                repr,
            }
        })
        .collect();
    let trivial = fields.iter().all(|f| is_trivial_repr(&f.repr));
    TupleRepr::to_machine_repr(fields, trivial)
}

/// Canonicalize a struct type with named fields.
fn canonical_struct(
    pool: &Pool,
    resolved: Idx,
    visiting: &mut FxHashSet<Idx>,
    cache: &mut FxHashMap<Idx, MachineRepr>,
) -> MachineRepr {
    let fields: Vec<FieldRepr> = pool
        .struct_fields(resolved)
        .into_iter()
        .enumerate()
        .map(|(i, (name, field_idx))| {
            let repr = canonical_inner(pool, field_idx, visiting, cache);
            let idx_u32 = u32::try_from(i).unwrap_or(u32::MAX);
            FieldRepr {
                name,
                original_index: idx_u32,
                offset: 0, // Set by §06 layout
                repr,
            }
        })
        .collect();
    let trivial = fields.iter().all(|f| is_trivial_repr(&f.repr));
    let (size, align) = compute_field_layout(&fields);
    MachineRepr::Struct(StructRepr {
        fields,
        size,
        align,
        trivial,
    })
}

/// Canonicalize an enum type with explicit i64 tag.
fn canonical_enum(
    pool: &Pool,
    resolved: Idx,
    visiting: &mut FxHashSet<Idx>,
    cache: &mut FxHashMap<Idx, MachineRepr>,
) -> MachineRepr {
    let variants: Vec<VariantRepr> = pool
        .enum_variants(resolved)
        .into_iter()
        .map(|(name, field_idxs)| {
            let fields: Vec<MachineRepr> = field_idxs
                .into_iter()
                .map(|fi| canonical_inner(pool, fi, visiting, cache))
                .collect();
            let (size, alignment) = compute_payload_layout(&fields);
            VariantRepr {
                name,
                fields,
                size,
                alignment,
            }
        })
        .collect();

    let max_payload = variants.iter().map(|v| v.size).max().unwrap_or(0);
    let max_align = variants
        .iter()
        .map(|v| v.alignment)
        .max()
        .unwrap_or(1)
        .max(8); // tag is i64 → 8-byte aligned
    let size = 8 + round_up(max_payload, max_align);

    MachineRepr::Enum(EnumRepr {
        tag: EnumTag::Explicit {
            width: IntWidth::I64,
        },
        variants,
        size,
        align: max_align,
    })
}

/// Build canonical `Option<T>` as a 2-variant enum: None (unit) + Some(T).
fn canonical_option(inner_repr: MachineRepr) -> MachineRepr {
    let none_variant = VariantRepr {
        name: Name::new(0, 0), // "None" — exact interning handled at call sites
        fields: vec![],
        size: 0,
        alignment: 1,
    };
    let some_size = field_size(&inner_repr);
    let some_align = field_align(&inner_repr);
    let some_variant = VariantRepr {
        name: Name::new(0, 1), // "Some"
        fields: vec![inner_repr],
        size: some_size,
        alignment: some_align,
    };

    let max_payload = some_size;
    let align = some_align.max(8); // i64 tag
    let size = 8 + round_up(max_payload, align);

    MachineRepr::Enum(EnumRepr {
        tag: EnumTag::Explicit {
            width: IntWidth::I64,
        },
        variants: vec![none_variant, some_variant],
        size,
        align,
    })
}

/// Build canonical `Result<T, E>` as a 2-variant enum: Ok(T) + Err(E).
fn canonical_result(ok_repr: MachineRepr, err_repr: MachineRepr) -> MachineRepr {
    let ok_size = field_size(&ok_repr);
    let ok_align = field_align(&ok_repr);
    let ok_variant = VariantRepr {
        name: Name::new(0, 0), // "Ok"
        fields: vec![ok_repr],
        size: ok_size,
        alignment: ok_align,
    };

    let err_size = field_size(&err_repr);
    let err_align = field_align(&err_repr);
    let err_variant = VariantRepr {
        name: Name::new(0, 1), // "Err"
        fields: vec![err_repr],
        size: err_size,
        alignment: err_align,
    };

    let max_payload = ok_size.max(err_size);
    let align = ok_align.max(err_align).max(8); // i64 tag
    let size = 8 + round_up(max_payload, align);

    MachineRepr::Enum(EnumRepr {
        tag: EnumTag::Explicit {
            width: IntWidth::I64,
        },
        variants: vec![ok_variant, err_variant],
        size,
        align,
    })
}
