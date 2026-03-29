//! Representation plan setup helpers for the codegen pipeline.
//!
//! Extracted from `codegen_pipeline.rs` to keep both files under 500 lines.
//! Contains:
//! - `collect_all_arc_functions`: flatten the (parent, lambdas) cache
//! - `lower_impl_methods_for_analysis`: ARC-lower impl methods for interprocedural repr analysis
//! - `compute_module_repr_plan`: build the repr plan from typed module metadata

use ori_ir::canon::CanonResult;
use ori_ir::ReprAttrKind;

use ori_types::{Idx, Pool, TypeCheckResult, Visibility};
use oric::ir::{Name, StringInterner};
use oric::parser::ParseOutput;
use rustc_hash::{FxHashMap, FxHashSet};

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

/// ARC-lower impl methods for interprocedural repr analysis.
///
/// The repr plan's range analysis needs to see call sites inside impl methods,
/// not just top-level functions. This lowers each non-generic impl method
/// (including default trait methods used in impl blocks) to ARC IR with
/// type-qualified names for disambiguation.
///
/// These ARC functions are for analysis only — `compile_impls()` in the
/// codegen pipeline does its own ARC lowering for LLVM emission.
pub(super) fn lower_impl_methods_for_analysis(
    parse_result: &ParseOutput,
    type_result: &TypeCheckResult,
    interner: &StringInterner,
    canon: &CanonResult,
    pool: &Pool,
) -> Vec<ori_arc::ArcFunction> {
    let mut funcs = Vec::new();
    let mut impl_arc_problems = Vec::new();
    // Ordinal counter: tracks how many times each (self_type, method_name)
    // pair has been seen, for disambiguating same-type same-name impls
    // like `impl Index<int, V>` and `impl Index<str, V>`.
    let mut method_ordinals: FxHashMap<(Idx, Name), usize> = FxHashMap::default();
    let mut sig_iter = type_result.typed.impl_sigs.iter();

    for impl_def in &parse_result.module.impls {
        // Resolve the self-type Name and Idx for this impl block.
        let type_name_name = impl_def.self_path.last().copied();
        let self_type_idx = type_name_name.and_then(|name| {
            type_result
                .typed
                .types
                .iter()
                .find(|te| te.name == name)
                .map(|te| te.idx)
        });

        for method in &impl_def.methods {
            let Some((_, sig)) = sig_iter.next() else {
                break;
            };
            if sig.is_generic() {
                continue;
            }
            let (ordinal, qualified_name) =
                make_qualified_name(self_type_idx, method.name, interner, &mut method_ordinals);
            let (arc_fn, lambdas) = if let Some(tn) = type_name_name {
                crate::arc_lowering::lower_impl_method_to_arc_nth(
                    qualified_name,
                    sig,
                    method.name,
                    tn,
                    ordinal,
                    canon,
                    interner,
                    pool,
                    &mut impl_arc_problems,
                    None,
                )
            } else {
                crate::arc_lowering::lower_to_arc(
                    qualified_name,
                    sig,
                    method.name,
                    canon,
                    interner,
                    pool,
                    &mut impl_arc_problems,
                    None,
                )
            };
            funcs.push(arc_fn);
            funcs.extend(lambdas);
        }

        // Skip default trait methods in sig_iter (they don't have
        // parse-level method definitions but are in impl_sigs).
        lower_default_trait_methods(
            impl_def,
            parse_result,
            &mut sig_iter,
            type_name_name,
            self_type_idx,
            interner,
            canon,
            pool,
            &mut method_ordinals,
            &mut impl_arc_problems,
            &mut funcs,
        );
    }
    funcs
}

/// Compute the ordinal-qualified name for an impl method.
///
/// Same-type same-name methods (e.g., two `impl Index<...>`) get ordinal
/// suffixes (`__impl_{idx}_{method}_{ordinal}`) for disambiguation.
fn make_qualified_name(
    self_type_idx: Option<Idx>,
    method_name: Name,
    interner: &StringInterner,
    method_ordinals: &mut FxHashMap<(Idx, Name), usize>,
) -> (usize, Name) {
    let ordinal = if let Some(idx) = self_type_idx {
        let entry = method_ordinals.entry((idx, method_name)).or_insert(0);
        let ord = *entry;
        *entry += 1;
        ord
    } else {
        0
    };
    let qualified = if let Some(idx) = self_type_idx {
        let method_str = interner.lookup(method_name);
        if ordinal == 0 {
            interner.intern(&format!("__impl_{}_{method_str}", idx.raw()))
        } else {
            interner.intern(&format!("__impl_{}_{}_{ordinal}", idx.raw(), method_str))
        }
    } else {
        method_name
    };
    (ordinal, qualified)
}

/// Lower default trait methods that appear in an impl block's sig list
/// but have no parse-level method definition (using the trait's default body).
#[expect(
    clippy::too_many_arguments,
    reason = "thread-through of pipeline state from the caller"
)]
fn lower_default_trait_methods<'a>(
    impl_def: &ori_ir::ImplDef,
    parse_result: &ParseOutput,
    sig_iter: &mut impl Iterator<Item = &'a (Name, ori_types::FunctionSig)>,
    type_name_name: Option<Name>,
    self_type_idx: Option<Idx>,
    interner: &StringInterner,
    canon: &CanonResult,
    pool: &Pool,
    method_ordinals: &mut FxHashMap<(Idx, Name), usize>,
    impl_arc_problems: &mut Vec<ori_arc::ArcProblem>,
    funcs: &mut Vec<ori_arc::ArcFunction>,
) {
    let Some(trait_path) = &impl_def.trait_path else {
        return;
    };
    let Some(&trait_name) = trait_path.last() else {
        return;
    };
    let overridden: FxHashSet<Name> = impl_def.methods.iter().map(|m| m.name).collect();
    let Some(trait_def) = parse_result
        .module
        .traits
        .iter()
        .find(|t| t.name == trait_name)
    else {
        return;
    };

    for item in &trait_def.items {
        if let ori_ir::TraitItem::DefaultMethod(default) = item {
            if !overridden.contains(&default.name) {
                let Some((_, sig)) = sig_iter.next() else {
                    break;
                };
                if sig.is_generic() {
                    continue;
                }
                let (ordinal, qualified_name) =
                    make_qualified_name(self_type_idx, default.name, interner, method_ordinals);
                let (arc_fn, lambdas) = if let Some(tn) = type_name_name {
                    crate::arc_lowering::lower_impl_method_to_arc_nth(
                        qualified_name,
                        sig,
                        default.name,
                        tn,
                        ordinal,
                        canon,
                        interner,
                        pool,
                        impl_arc_problems,
                        None,
                    )
                } else {
                    crate::arc_lowering::lower_to_arc(
                        qualified_name,
                        sig,
                        default.name,
                        canon,
                        interner,
                        pool,
                        impl_arc_problems,
                        None,
                    )
                };
                funcs.push(arc_fn);
                funcs.extend(lambdas);
            }
        }
    }
}

/// Build the representation plan from a type-checked module.
///
/// Extracts `#repr` attributes and public type indices from the typed module,
/// then runs the repr plan computation pipeline (canonical reprs, range analysis,
/// integer narrowing, float narrowing).
///
/// `imported_type_metadata` carries repr/pub metadata from imported modules,
/// enabling the repr plan to correctly exempt imported `pub` and `#repr(...)`
/// types from integer narrowing.
///
/// Must run AFTER borrow inference (accepts `ArcFunction`s for range analysis)
/// and BEFORE codegen (`TypeLayoutResolver` and `TypeInfoStore` read the plan).
#[expect(
    clippy::too_many_arguments,
    reason = "each parameter carries distinct metadata from different compiler phases"
)]
pub(super) fn compute_module_repr_plan(
    pool: &Pool,
    all_arc_funcs: &[ori_arc::ArcFunction],
    narrowing_policy: ori_repr::NarrowingPolicy,
    type_result: &TypeCheckResult,
    interner: Option<&StringInterner>,
    imported_type_metadata: &[ori_types::ExportedTypeMetadata],
    imported_collection_surfaces: &[u64],
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
    // that integer narrowing must not violate.
    let mut pub_type_indices: Vec<Idx> = type_result
        .typed
        .types
        .iter()
        .filter(|te| te.visibility == Visibility::Public)
        .map(|te| te.idx)
        .collect();

    // Also mark collection wrapper types from public function signatures as
    // public. A public `@f (xs: [int]) -> [int]` means the `[int]` type's
    // element layout is an ABI surface — callers construct lists with canonical
    // element widths. Without this, integer narrowing Phase C could narrow the
    // element repr while callers still use 8-byte strides.
    collect_public_collection_types(pool, &type_result.typed.functions, &mut pub_type_indices);

    // Collect unconstrained function names (pub + trait impl) for interprocedural range analysis.
    let unconstrained_fn_names = collect_unconstrained_fn_names(
        &type_result.typed.functions,
        &type_result.typed.trait_impl_fn_names,
        interner,
    );

    ori_repr::compute_repr_plan_with_interner(
        pool,
        all_arc_funcs,
        narrowing_policy,
        &repr_attrs,
        interner,
        &pub_type_indices,
        imported_type_metadata,
        imported_collection_surfaces,
        &unconstrained_fn_names,
        has_analysis_only_functions,
    )
}

/// Collect unconstrained function identities (pub or trait impl).
///
/// These functions may be called from external code or via dynamic dispatch,
/// so interprocedural range analysis must assign Top to their parameters.
/// Only trait impl methods are included — inherent impl methods have known
/// call sites and can be narrowed.
///
/// Returns `(Option<Idx>, Name)` pairs: `None` for pub top-level functions,
/// `Some(self_type)` for trait impl methods (for disambiguation).
///
/// Tracks ordinals for same-type same-name method duplicates (e.g., two
/// `impl Index<...>` on the same type). Registers both the base qualified
/// name (`__impl_{idx}_{method}`) and ordinal-suffixed variants
/// (`__impl_{idx}_{method}_{ordinal}`) to match the analysis-only ARC
/// lowering format.
pub(super) fn collect_unconstrained_fn_names(
    function_sigs: &[ori_types::FunctionSig],
    trait_impl_fn_names: &[(ori_types::Idx, oric::ir::Name)],
    interner: Option<&oric::ir::StringInterner>,
) -> Vec<(Option<ori_types::Idx>, oric::ir::Name)> {
    let mut names = Vec::new();
    // Public top-level functions — external callers may pass any value.
    for sig in function_sigs {
        if sig.is_public {
            names.push((None, sig.name));
        }
    }
    // Trait impl methods only — may be called via dynamic dispatch.
    // Inherent impl methods are NOT included — they have known call sites.
    // Carries self-type for disambiguation.
    // Tracks ordinals for same-type same-name duplicates.
    let mut method_ordinals: FxHashMap<(ori_types::Idx, oric::ir::Name), usize> =
        FxHashMap::default();
    for &(self_type, name) in trait_impl_fn_names {
        names.push((Some(self_type), name));
        // Register the qualified name used by analysis-only ARC functions.
        // The format matches what codegen_pipeline and the JIT arc_lowering use
        // when ARC-lowering impl methods for range analysis.
        // Ordinal-qualified names are registered for same-type same-name
        // duplicates so `is_qualified_unconstrained()` finds them.
        if let Some(interner) = interner {
            let ordinal = {
                let entry = method_ordinals.entry((self_type, name)).or_insert(0);
                let ord = *entry;
                *entry += 1;
                ord
            };
            let method_str = interner.lookup(name);
            let qualified = if ordinal == 0 {
                interner.intern(&format!("__impl_{}_{method_str}", self_type.raw()))
            } else {
                interner.intern(&format!(
                    "__impl_{}_{}_{ordinal}",
                    self_type.raw(),
                    method_str
                ))
            };
            names.push((None, qualified));
        }
    }
    names
}

/// Collect collection wrapper type indices from public function signatures.
///
/// When a public function has `[int]`, `Set<int>`, or similar collection
/// types in its parameters or return type, those collection type indices
/// must be marked public so integer narrowing Phase C does not narrow their
/// element layout (which would break ABI with external callers).
///
/// Uses the shared `walk_collection_types` walker from `ori_types::pool`
/// to avoid duplicating the recursive type-walking logic.
fn collect_public_collection_types(
    pool: &Pool,
    function_sigs: &[ori_types::FunctionSig],
    pub_type_indices: &mut Vec<Idx>,
) {
    for sig in function_sigs {
        if !sig.is_public {
            continue;
        }
        for &param_ty in &sig.param_types {
            ori_types::walk_collection_types(pool, param_ty, &mut |idx| {
                if !pub_type_indices.contains(&idx) {
                    pub_type_indices.push(idx);
                }
            });
        }
        ori_types::walk_collection_types(pool, sig.return_type, &mut |idx| {
            if !pub_type_indices.contains(&idx) {
                pub_type_indices.push(idx);
            }
        });
    }
}
