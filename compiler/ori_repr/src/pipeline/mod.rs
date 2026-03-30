//! Representation optimization pipeline.
//!
//! Contains the main entry points [`compute_repr_plan()`] and
//! [`compute_repr_plan_with_interner()`], plus all pipeline phases
//! (triviality validation, range analysis, integer/float narrowing,
//! struct layout, enum repr, escape analysis, ARC header compression,
//! thread-local ARC, and collection specialization).

mod metadata;

use ori_arc::ir::ArcFunction;
use ori_ir::ReprAttrKind;
use ori_types::{Idx, Pool};

use crate::plan::{NarrowingPolicy, ReprAttribute, ReprPlan};
use crate::{narrowing, range, DecisionReason, DecisionSource, MachineRepr, ReprDecision};

use metadata::{propagate_metadata_to_applied_resolutions, seed_imported_metadata};

/// Compute the representation plan for all types reachable from the program.
///
/// Called after type checking and ARC borrow inference, before LLVM codegen.
/// The `arc_functions` parameter is consumed by the range analysis and escape
/// analysis passes.
///
/// `repr_attrs` carries user-specified `#repr` attributes from the type
/// registry, keyed by pool `Idx`. Each entry is converted to
/// `ReprAttribute` and stored in the plan for struct layout and enum repr
/// passes to query.
///
/// When `policy` is `NarrowingPolicy::Disabled` (`--no-repr-opt`), returns
/// after `populate_canonical()` — canonical representations only, zero
/// behavioral change versus the pre-`ori_repr` pipeline.
pub fn compute_repr_plan(
    pool: &Pool,
    arc_functions: &[ArcFunction],
    policy: NarrowingPolicy,
    repr_attrs: &[(Idx, ReprAttrKind)],
) -> ReprPlan {
    compute_repr_plan_with_interner(
        pool,
        arc_functions,
        policy,
        repr_attrs,
        None,
        &[],
        &[],
        &[],
        &[],
        false,
    )
}

/// Compute the representation plan with access to the string interner.
///
/// When `interner` is `Some`, builtin function names (len, abs, etc.) are
/// resolved for range analysis, enabling tighter ranges on builtin calls.
/// When `None`, builtin ranges conservatively degrade to `Top`.
///
/// `imported_type_metadata` carries repr/pub metadata from imported modules.
/// Each entry's `merkle_hash` is mapped to a local `Idx` via `Pool::lookup_by_hash()`
/// to seed the plan with imported type protections.
///
/// `unconstrained_fn_names` lists function `Name`s whose parameters must not
/// be narrowed (pub functions, trait impl methods). Closures are detected
/// directly from `ArcFunction::num_captures`.
#[expect(
    clippy::too_many_arguments,
    reason = "each parameter carries distinct metadata from different compiler phases"
)]
pub fn compute_repr_plan_with_interner(
    pool: &Pool,
    arc_functions: &[ArcFunction],
    policy: NarrowingPolicy,
    repr_attrs: &[(Idx, ReprAttrKind)],
    interner: Option<&ori_ir::StringInterner>,
    pub_type_indices: &[Idx],
    imported_type_metadata: &[ori_types::ExportedTypeMetadata],
    imported_collection_surfaces: &[u64],
    unconstrained_fn_names: &[(Option<Idx>, ori_ir::Name)],
    has_analysis_only_functions: bool,
) -> ReprPlan {
    let mut plan = ReprPlan::new(policy);
    if has_analysis_only_functions {
        plan.set_has_analysis_only_functions();
    }

    // Phase 0: Store user-specified #repr attributes.
    // Also store under the resolved idx so that narrowing and codegen (which
    // operate on resolved struct/tuple indices) see the attr.
    for &(idx, ref attr) in repr_attrs {
        let converted = convert_repr_attr_kind(attr);
        plan.set_repr_attr(idx, converted);
        let resolved = pool.resolve_fully(idx);
        if resolved != idx {
            plan.set_repr_attr(resolved, converted);
        }
    }

    // Phase 0a-import: Seed imported repr/pub/collection metadata.
    let (imported_repr_attrs, imported_pub_indices, _imported_collection_indices) =
        seed_imported_metadata(
            &mut plan,
            pool,
            imported_type_metadata,
            imported_collection_surfaces,
        );

    // Phase 0b: Store public type indices for ABI-safe narrowing.
    // Also store under the resolved idx for the same reason.
    // Includes local public types and imported public user-defined types.
    //
    // NOTE: Imported collection surfaces are NOT added here.
    // They exist for transitive forwarding metadata (A→B→C), not for narrowing
    // suppression. Same-module public functions already mark `[int]` as public
    // via `collect_public_collection_types()`. Imported surfaces would suppress
    // narrowing for private `[int]` usage that never crosses module boundaries,
    // because pool interning deduplicates `List<int>` to one Idx. The imported
    // collection indices are still resolved by `seed_imported_metadata()` for
    // future use when cross-module ABI widening is implemented.
    plan.set_pub_type_indices(
        pub_type_indices
            .iter()
            .flat_map(|&idx| {
                let resolved = pool.resolve_fully(idx);
                if resolved == idx {
                    vec![idx]
                } else {
                    vec![idx, resolved]
                }
            })
            .chain(imported_pub_indices),
    );

    // Phase 0b2: Store unconstrained function names for interprocedural range analysis.
    // Public functions and trait impl methods have parameters that could be
    // called with any value — their parameter ranges must stay Top.
    plan.set_unconstrained_fn_names(unconstrained_fn_names.iter().copied());

    // Phase 0c: Propagate repr/pub metadata through Applied → concrete Struct
    // resolutions for monomorphized generic types.
    //
    // Phases 0/0b store metadata on the Named idx and its direct resolve_fully()
    // result. But monomorphization creates additional Applied(Name, [ConcreteArgs])
    // → Struct resolutions that aren't in the Named chain. We collect the set of
    // protected type Names, then scan the pool for Applied entries whose name
    // matches. Each matching Applied is resolved through the pool, and the
    // concrete Struct idx receives the same repr/pub metadata.
    //
    // Combine local and imported repr attrs/pub indices for propagation.
    let all_repr_attrs: Vec<(Idx, ReprAttrKind)> = repr_attrs
        .iter()
        .copied()
        .chain(imported_repr_attrs)
        .collect();
    let all_pub_indices: Vec<Idx> = pub_type_indices
        .iter()
        .copied()
        .chain(
            imported_type_metadata
                .iter()
                .filter(|m| m.is_public)
                .filter_map(|m| pool.lookup_by_hash(m.merkle_hash)),
        )
        .collect();
    propagate_metadata_to_applied_resolutions(&mut plan, pool, &all_repr_attrs, &all_pub_indices);

    // Phase 1: Set canonical representations for all types.
    crate::canonical::populate_canonical(&mut plan, pool);

    if policy == NarrowingPolicy::Disabled {
        tracing::debug!("repr-opt disabled — returning canonical-only plan");
        return plan;
    }

    // Phase 2: Triviality analysis
    analyze_triviality(&mut plan, pool);

    // Phase 3: Range analysis → Integer narrowing → Float narrowing
    analyze_ranges(&mut plan, pool, arc_functions, interner);
    apply_integer_narrowing(&mut plan, pool);
    apply_float_narrowing(&mut plan, pool, arc_functions);

    // Phase 4: Struct layout, Enum repr
    compute_struct_layouts(&mut plan, pool);
    compute_enum_reprs(&mut plan, pool);

    // Phase 5: Escape analysis → ARC header compression → Thread-local ARC
    analyze_escape(&mut plan, pool, arc_functions);
    compress_arc_headers(&mut plan, pool);
    apply_thread_local_arc(&mut plan, pool, arc_functions);

    // Phase 6: Collection specialization
    specialize_collections(&mut plan, pool);

    plan
}

// Phase 0a-import and Phase 0c metadata propagation live in `metadata.rs`.

// Repr pipeline passes — active implementations and future stubs.

/// Transitive triviality validation pass.
///
/// Validates that `classify_triviality()` (the canonical source of truth in
/// `ori_types`) agrees with `is_trivial_repr()` for every type that has a
/// stored canonical representation in the `ReprPlan`. Types without a stored
/// repr are skipped (they weren't canonicalized — typically unresolved or
/// error types that don't reach codegen).
///
/// This is a **validation-only** pass — it does not modify the `ReprPlan`.
/// The canonical pass (`populate_canonical()`) already embeds the correct
/// triviality into `StructRepr.trivial`, `TupleRepr.trivial`, and enum
/// variant walks. Any mismatch is a bug in either `classify_triviality()`
/// or `populate_canonical()`.
fn analyze_triviality(plan: &mut ReprPlan, pool: &Pool) {
    use ori_types::triviality::{classify_triviality, Triviality};

    let pool_len = u32::try_from(pool.len()).unwrap_or(u32::MAX);
    let mut validated: u32 = 0;
    let mut mismatches: u32 = 0;

    for raw in 0..pool_len {
        let idx = ori_types::Idx::from_raw(raw);

        // Skip types without a stored repr — they weren't canonicalized.
        let Some(repr) = plan.get_repr(idx) else {
            continue;
        };

        let pool_triviality = classify_triviality(idx, pool);
        let repr_trivial = crate::layout::is_trivial_repr(repr);

        match pool_triviality {
            Triviality::Trivial if !repr_trivial => {
                tracing::warn!(
                    ?idx,
                    "triviality mismatch: classify_triviality says Trivial, ReprPlan says non-trivial"
                );
                mismatches += 1;
            }
            Triviality::NonTrivial if repr_trivial => {
                tracing::warn!(
                    ?idx,
                    "triviality mismatch: classify_triviality says NonTrivial, ReprPlan says trivial"
                );
                mismatches += 1;
            }
            _ => {} // Agrees, or Unknown (can't validate — conservative OK)
        }
        validated += 1;
    }

    tracing::debug!(validated, mismatches, "triviality validation complete");
    debug_assert_eq!(
        mismatches, 0,
        "triviality classification disagrees with canonical repr for {mismatches} type(s)"
    );
}

/// Value range analysis (interval propagation per function).
///
/// Runs interprocedural range propagation: intraprocedural fixpoint per
/// function, SCC-based parameter seeding, and return-range propagation.
/// Results stored in `ReprPlan::function_var_ranges` and `field_range_summaries`.
///
/// When `interner` is provided, builtin function names are resolved for
/// tighter range analysis on builtin calls.
fn analyze_ranges(
    plan: &mut ReprPlan,
    pool: &Pool,
    arc_functions: &[ArcFunction],
    interner: Option<&ori_ir::StringInterner>,
) {
    let mut config = range::RangeAnalysisConfig::default();
    if let Some(interner) = interner {
        config.known_builtins = range::KnownBuiltins::from_interner(interner);
    }
    range::propagate_ranges(plan, pool, arc_functions, &config);
}

/// Integer narrowing (i64 → i32/i16/i8 when range fits).
///
/// Phase A: Struct/tuple field narrowing from field-range summaries.
/// Phase B: Local variable narrowing + overflow guards.
/// Phase C: Collection element narrowing.
fn apply_integer_narrowing(plan: &mut ReprPlan, pool: &Pool) {
    // Gate: only narrow when the full integer narrowing pipeline is safe for codegen.
    // Narrowing produces MachineRepr::Int widths that codegen consumes for
    // struct layouts, trunc/sext, phi nodes, and collection GEP strides.
    // Range analysis is always safe — it only populates
    // function_var_ranges, field_range_summaries, and element_range_summaries.
    if !plan.is_narrowing_safe_for_codegen() {
        return;
    }
    narrowing::int::narrow_struct_fields(plan, pool);
    narrowing::int::narrow_collection_elements(plan, pool);
}

/// Float narrowing (f64 → f32 when precision is exact).
///
/// Phase A: Storage-only narrowing — struct fields where ALL observed
/// values are f32-exact literals. Arithmetic results are never narrowed.
fn apply_float_narrowing(plan: &mut ReprPlan, pool: &Pool, arc_functions: &[ArcFunction]) {
    if plan.narrowing_policy() == NarrowingPolicy::Disabled {
        return;
    }
    if !plan.is_narrowing_safe_for_codegen() {
        return;
    }
    let float_table = narrowing::float::collect_float_field_summaries(plan, pool, arc_functions);
    narrowing::float::narrow_float_fields(plan, pool, &float_table);
}

/// Struct and tuple field reordering for padding minimization.
///
/// Iterates all struct/tuple types in the `ReprPlan`, applies the field
/// reordering algorithm (descending alignment, then descending size),
/// and writes the optimized layout back. `#repr("c")`, `#repr("packed")`,
/// and `#repr("transparent")` structs are handled with their own layout
/// strategies (no reordering for C, no padding for packed, etc.).
fn compute_struct_layouts(plan: &mut ReprPlan, pool: &Pool) {
    use crate::layout::struct_layout::optimize_struct_layout;

    let indices: Vec<Idx> = plan.decision_indices().collect();
    // Collect (idx, optimized_repr) to write back after iteration.
    let mut updates: Vec<(Idx, MachineRepr)> = Vec::new();

    for idx in indices {
        let Some(repr) = plan.get_repr(idx) else {
            continue;
        };
        // Only reorder structs. Tuple reordering is deferred: tuples cross
        // the runtime boundary in map iteration (runtime writes key+value
        // in declaration order).
        if let MachineRepr::Struct(s) = repr {
            let attr = plan.repr_attr(idx);
            let optimized = optimize_struct_layout(s, attr);
            updates.push((idx, MachineRepr::Struct(optimized)));
        }
    }

    // Write back reordered layouts. Also propagate to resolved aliases
    // so that different Pool Idx values for the same logical type (common
    // with monomorphized generics) share the same reordered layout.
    for (idx, repr) in updates {
        let decision = ReprDecision {
            source: DecisionSource::StructLayout,
            type_idx: idx,
            repr: repr.clone(),
            reason: DecisionReason::Custom("field reordering".into()),
        };
        plan.set_repr(idx, decision);

        // Propagate to all Pool entries that resolve to this type.
        // Monomorphization creates multiple Idx values for the same
        // concrete type — without propagation, call sites may use a
        // different Idx than the function body, causing layout mismatch.
        propagate_layout_to_aliases(plan, pool, idx, &repr);
    }
}

/// Propagate a reordered layout to all Pool entries that share the same
/// concrete type structure. This handles the Pool aliasing issue where
/// monomorphized generics create multiple Idx values for the same logical type.
fn propagate_layout_to_aliases(
    plan: &mut ReprPlan,
    pool: &Pool,
    source_idx: Idx,
    repr: &MachineRepr,
) {
    let pool_len = u32::try_from(pool.len()).unwrap_or(u32::MAX);
    let source_tag = pool.tag(source_idx);

    // Only propagate tuples and structs.
    if source_tag != ori_types::Tag::Tuple && source_tag != ori_types::Tag::Struct {
        return;
    }

    // Get the structural signature of the source type (resolved element types).
    let source_elems: Vec<Idx> = if source_tag == ori_types::Tag::Tuple {
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
        let elems: Vec<Idx> = if source_tag == ori_types::Tag::Tuple {
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
            // Same structural type — propagate the reordered layout.
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
        ori_types::Tag::Tuple => {
            let a_elems = pool.tuple_elems(a);
            let b_elems = pool.tuple_elems(b);
            a_elems.len() == b_elems.len()
                && a_elems
                    .iter()
                    .zip(b_elems.iter())
                    .all(|(&x, &y)| structural_type_eq(pool, x, y))
        }
        ori_types::Tag::Struct => {
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
        ori_types::Tag::Option => {
            structural_type_eq(pool, pool.option_inner(a), pool.option_inner(b))
        }
        ori_types::Tag::Result => {
            structural_type_eq(pool, pool.result_ok(a), pool.result_ok(b))
                && structural_type_eq(pool, pool.result_err(a), pool.result_err(b))
        }
        ori_types::Tag::List => structural_type_eq(pool, pool.list_elem(a), pool.list_elem(b)),
        ori_types::Tag::Set => structural_type_eq(pool, pool.set_elem(a), pool.set_elem(b)),
        ori_types::Tag::Map => {
            structural_type_eq(pool, pool.map_key(a), pool.map_key(b))
                && structural_type_eq(pool, pool.map_value(a), pool.map_value(b))
        }
        ori_types::Tag::Iterator | ori_types::Tag::DoubleEndedIterator => {
            structural_type_eq(pool, pool.iterator_elem(a), pool.iterator_elem(b))
        }
        // Primitive/leaf tags (Int, Float, Bool, etc.): same tag + different
        // resolved Idx should not happen (singletons). If it does, reject the
        // match — the Idx equality check at the top already handles true equality.
        _ => false,
    }
}

/// Enum niche optimization and discriminant narrowing.
fn compute_enum_reprs(_plan: &mut ReprPlan, _pool: &Pool) {}

/// Escape analysis for stack promotion.
fn analyze_escape(_plan: &mut ReprPlan, _pool: &Pool, _fns: &[ArcFunction]) {}

/// ARC header compression (refcount width narrowing).
fn compress_arc_headers(_plan: &mut ReprPlan, _pool: &Pool) {}

/// Thread-local non-atomic ARC (Rc vs Arc selection).
fn apply_thread_local_arc(_plan: &mut ReprPlan, _pool: &Pool, _fns: &[ArcFunction]) {}

/// Collection specialization (SSO, SVO, packed bool, element narrowing).
fn specialize_collections(_plan: &mut ReprPlan, _pool: &Pool) {}

/// Convert IR-level `ReprAttrKind` to repr-opt `ReprAttribute`.
pub(crate) fn convert_repr_attr_kind(kind: &ReprAttrKind) -> ReprAttribute {
    match *kind {
        ReprAttrKind::C => ReprAttribute::C,
        ReprAttrKind::Packed => ReprAttribute::Packed,
        ReprAttrKind::Transparent => ReprAttribute::Transparent,
        ReprAttrKind::Aligned(n) => ReprAttribute::Aligned(u32::try_from(n).unwrap_or(u32::MAX)),
        ReprAttrKind::CAligned(n) => ReprAttribute::CAligned(u32::try_from(n).unwrap_or(u32::MAX)),
    }
}
