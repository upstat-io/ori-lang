//! Representation plan setup helpers for the codegen pipeline.
//!
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
/// Flatten the ARC cache into a single owned `Vec` for consumption by
/// downstream passes (repr plan, uniqueness analysis, AIMS contracts).
///
/// Delegates to [`ori_arc::collect_all_arc_functions`] — the single
/// canonical implementation of this flattening algorithm.
pub(super) fn collect_all_arc_functions(
    arc_cache: &FxHashMap<Name, (ori_arc::ArcFunction, Vec<ori_arc::ArcFunction>)>,
) -> Vec<ori_arc::ArcFunction> {
    ori_arc::collect_all_arc_functions(arc_cache)
}

/// ARC-lower impl methods for interprocedural repr + AIMS-contract analysis.
///
/// The repr plan's range analysis and the AIMS interprocedural contract
/// computation both need to see impl-method bodies + call sites, not just
/// top-level functions. This lowers each non-generic impl method (including
/// default trait methods used in impl blocks) to ARC IR with type-qualified
/// names for disambiguation.
///
/// Returns the lowered `ArcFunctions` plus a `(self_type_idx, method_name)
/// -> qualified_name` map. The map keys the as-compiled impl-method
/// contracts (`ori_arc::compute_impl_method_contracts`) that
/// `ori_arc::augment_contracts_with_impl_callees` binds per caller function
/// at Phase-5, exposing `transfers_through_return` for impl-method callees.
///
/// These ARC functions are for analysis only — `compile_impls()` in the
/// codegen pipeline does its own ARC lowering for LLVM emission.
pub(super) fn lower_impl_methods_for_analysis(
    parse_result: &ParseOutput,
    type_result: &TypeCheckResult,
    interner: &StringInterner,
    canon: &CanonResult,
    pool: &Pool,
) -> (Vec<ori_arc::ArcFunction>, FxHashMap<(Idx, Name), Name>) {
    let mut funcs = Vec::new();
    // (self_type_idx, method_name) -> qualified analysis name, ordinal-0 entries
    // only (the receiver-type-disambiguated common case; same-name multi-impl
    // ordinals are disambiguated by key-type at emission, out of scope here).
    let mut qualified_by_recv: FxHashMap<(Idx, Name), Name> = FxHashMap::default();
    let mut impl_arc_problems = Vec::new();
    // Ordinal counter: tracks how many times each (self_type, method_name)
    // pair has been seen, for disambiguating same-type same-name impls
    // like `impl Index<int, V>` and `impl Index<str, V>`.
    let mut method_ordinals: FxHashMap<(Idx, Name), usize> = FxHashMap::default();
    let mut sig_iter = type_result.typed.impl_sigs.iter();

    for impl_def in &parse_result.module.impls {
        // Resolve the self-type Name and Idx for this impl block.
        let type_name_name = impl_def.type_name();
        let self_type_idx = type_name_name.and_then(|name| {
            type_result
                .typed
                .types
                .iter()
                .find(|te| te.name == name)
                .map(|te| te.idx)
        });

        for method in &impl_def.methods {
            let Some(ori_types::ImplSig { sig, .. }) = sig_iter.next() else {
                break;
            };
            if sig.is_generic() {
                continue;
            }
            let (ordinal, qualified_name) =
                make_qualified_name(self_type_idx, method.name, interner, &mut method_ordinals);
            // Key by the resolved self-param type — the same index the
            // caller's receiver (`args[0]`) resolves to (and the index
            // emission keys `type_idx_to_name` on), NOT the TypeEntry idx.
            let recv_key = sig.param_types.first().map(|&t| pool.resolve_fully(t));
            record_qualified_by_recv(
                &mut qualified_by_recv,
                recv_key,
                method.name,
                ordinal,
                qualified_name,
            );
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
            &mut qualified_by_recv,
        );
    }
    (funcs, qualified_by_recv)
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

/// Record the `(self_type, method) -> qualified` analysis-name mapping.
///
/// Ordinal 0 inserts; ordinal > 0 REMOVES the key — two same-name impls on
/// one type (e.g. `Index<int>` + `Index<str>`) make the `(type, method)` key
/// ambiguous, so receiver-based contract binding must decline it entirely
/// (binding ordinal 0's contract to call sites that dispatch to ordinal 1
/// would consult the wrong contract).
fn record_qualified_by_recv(
    qualified_by_recv: &mut FxHashMap<(Idx, Name), Name>,
    recv_key_idx: Option<Idx>,
    method_name: Name,
    ordinal: usize,
    qualified_name: Name,
) {
    let Some(idx) = recv_key_idx else {
        return;
    };
    if ordinal == 0 {
        qualified_by_recv.insert((idx, method_name), qualified_name);
    } else {
        qualified_by_recv.remove(&(idx, method_name));
    }
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
    sig_iter: &mut impl Iterator<Item = &'a ori_types::ImplSig>,
    type_name_name: Option<Name>,
    self_type_idx: Option<Idx>,
    interner: &StringInterner,
    canon: &CanonResult,
    pool: &Pool,
    method_ordinals: &mut FxHashMap<(Idx, Name), usize>,
    impl_arc_problems: &mut Vec<ori_arc::ArcProblem>,
    funcs: &mut Vec<ori_arc::ArcFunction>,
    qualified_by_recv: &mut FxHashMap<(Idx, Name), Name>,
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
                let Some(ori_types::ImplSig { sig, .. }) = sig_iter.next() else {
                    break;
                };
                if sig.is_generic() {
                    continue;
                }
                let (ordinal, qualified_name) =
                    make_qualified_name(self_type_idx, default.name, interner, method_ordinals);
                let recv_key = sig.param_types.first().map(|&t| pool.resolve_fully(t));
                record_qualified_by_recv(
                    qualified_by_recv,
                    recv_key,
                    default.name,
                    ordinal,
                    qualified_name,
                );
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
    ori_types::collect_public_collection_types(
        pool,
        &type_result.typed.functions,
        &mut pub_type_indices,
    );

    // Collect unconstrained function names (pub + trait impl) for
    // interprocedural range analysis. The qualified-name algorithm is a
    // cross-phase contract feeding compute_repr_plan_with_interner — the
    // canonical implementation lives in ori_llvm.
    let unconstrained_fn_names = ori_llvm::collect_unconstrained_fn_names(
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
