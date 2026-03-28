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
/// `imported_type_metadata` carries repr/pub metadata from imported modules,
/// enabling the repr plan to correctly exempt imported `pub` and `#repr(...)`
/// types from integer narrowing. See CROSS-04-014.
///
/// Must run AFTER borrow inference (accepts `ArcFunction`s for §03 range analysis)
/// and BEFORE codegen (`TypeLayoutResolver` and `TypeInfoStore` read the plan).
pub(super) fn compute_module_repr_plan(
    pool: &Pool,
    all_arc_funcs: &[ori_arc::ArcFunction],
    narrowing_policy: ori_repr::NarrowingPolicy,
    type_result: &TypeCheckResult,
    interner: Option<&StringInterner>,
    imported_type_metadata: &[ori_types::ExportedTypeMetadata],
    has_analysis_only_functions: bool,
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

    // Collect unconstrained function names (pub + trait impl) for §03.5.
    let unconstrained_fn_names = collect_unconstrained_fn_names(
        &type_result.typed.functions,
        &type_result.typed.trait_impl_fn_names,
    );

    ori_repr::compute_repr_plan_with_interner(
        pool,
        all_arc_funcs,
        narrowing_policy,
        &repr_attrs,
        interner,
        &pub_type_indices,
        imported_type_metadata,
        &unconstrained_fn_names,
        has_analysis_only_functions,
    )
}

/// Collect unconstrained function identities (pub or trait impl).
///
/// These functions may be called from external code or via dynamic dispatch,
/// so §03.5 interprocedural range analysis must assign Top to their parameters.
/// Only trait impl methods are included — inherent impl methods have known
/// call sites and can be narrowed (TPR-03-038).
///
/// Returns `(Option<Idx>, Name)` pairs: `None` for pub top-level functions,
/// `Some(self_type)` for trait impl methods (TPR-03-042 disambiguation).
pub(super) fn collect_unconstrained_fn_names(
    function_sigs: &[ori_types::FunctionSig],
    trait_impl_fn_names: &[(ori_types::Idx, oric::ir::Name)],
) -> Vec<(Option<ori_types::Idx>, oric::ir::Name)> {
    let mut names = Vec::new();
    // Public top-level functions — external callers may pass any value.
    for sig in function_sigs {
        if sig.is_public {
            names.push((None, sig.name));
        }
    }
    // Trait impl methods only — may be called via dynamic dispatch.
    // Inherent impl methods are NOT included (TPR-03-038).
    // Carries self-type for disambiguation (TPR-03-042).
    for &(self_type, name) in trait_impl_fn_names {
        names.push((Some(self_type), name));
    }
    names
}
