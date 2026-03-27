//! Representation plan setup helpers for the codegen pipeline.
//!
//! Extracted from `codegen_pipeline.rs` to keep both files under 500 lines.
//! Contains:
//! - `collect_all_arc_functions`: flatten the (parent, lambdas) cache
//! - `compute_module_repr_plan`: build the repr plan from typed module metadata

use ori_ir::ReprAttrKind;

use ori_types::{Idx, Pool, TypeCheckResult, Visibility};
use oric::ir::{Name, StringInterner};
use rustc_hash::FxHashMap;

/// Collect all ARC functions from the inference cache (parents + lambdas).
///
/// The arc cache maps each top-level function name to `(parent, lambdas)`.
/// This flattens the cache into a single owned `Vec` for consumption by
/// downstream passes (repr plan, uniqueness analysis, AIMS contracts).
pub(super) fn collect_all_arc_functions(
    arc_cache: &FxHashMap<Name, (ori_arc::ArcFunction, Vec<ori_arc::ArcFunction>)>,
) -> Vec<ori_arc::ArcFunction> {
    arc_cache
        .values()
        .flat_map(|(parent, lambdas)| std::iter::once(parent).chain(lambdas.iter()))
        .cloned()
        .collect()
}

/// Build the representation plan from a type-checked module.
///
/// Extracts `#repr` attributes and public type indices from the typed module,
/// then runs the repr plan computation pipeline (§01 canonical reprs, §03 range
/// analysis, §04 integer narrowing).
///
/// Must run AFTER borrow inference (accepts `ArcFunction`s for §03 range analysis)
/// and BEFORE codegen (`TypeLayoutResolver` and `TypeInfoStore` read the plan).
pub(super) fn compute_module_repr_plan(
    pool: &Pool,
    all_arc_funcs: &[ori_arc::ArcFunction],
    narrowing_policy: ori_repr::NarrowingPolicy,
    type_result: &TypeCheckResult,
    interner: Option<&StringInterner>,
) -> ori_repr::ReprPlan {
    // Extract #repr attributes from typed module for the repr plan.
    let repr_attrs: Vec<(Idx, ReprAttrKind)> = type_result
        .typed
        .types
        .iter()
        .filter_map(|te| te.repr.map(|r| (te.idx, r)))
        .collect();

    // Extract public type indices — their field layout is an ABI contract
    // that §04 integer narrowing must not violate (TPR-04-005).
    let pub_type_indices: Vec<Idx> = type_result
        .typed
        .types
        .iter()
        .filter(|te| te.visibility == Visibility::Public)
        .map(|te| te.idx)
        .collect();

    ori_repr::compute_repr_plan_with_interner(
        pool,
        all_arc_funcs,
        narrowing_policy,
        &repr_attrs,
        interner,
        &pub_type_indices,
    )
}
