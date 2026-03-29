//! ARC lowering and borrow inference for the LLVM JIT test runner.
//!
//! Lowers all functions (local, imported, impl methods, monomorphized) to ARC IR
//! and runs borrow inference to produce annotated signatures and a pre-lowered cache.

use rustc_hash::FxHashMap;

use ori_ir::Name;

/// Result of ARC lowering: annotated borrow signatures + per-function cache.
pub(crate) type ArcLoweringResult = (
    FxHashMap<Name, ori_arc::AnnotatedSig>,
    FxHashMap<Name, (ori_arc::ArcFunction, Vec<ori_arc::ArcFunction>)>,
);

/// Lower all functions to ARC IR and run borrow inference.
///
/// This is the analysis phase that was previously embedded in the evaluator.
/// It lowers each function exactly once, runs `infer_borrows_scc` on the flat
/// list, then builds a cache mapping `Name → (ArcFunction, Vec<ArcFunction>)`
/// for zero-copy consumption by `prepare_all_cached`.
///
/// Functions lowered: module functions, imported functions, impl methods,
/// and monomorphized generic functions.
///
/// Maps function names to their ARC-lowered forms with borrow annotations.
#[expect(
    clippy::too_many_arguments,
    reason = "mirrors the data flow from run_file_llvm — all inputs are required"
)]
#[expect(
    clippy::too_many_lines,
    reason = "ARC lowering pipeline — local, imported, impl, and mono functions in sequence"
)]
pub(crate) fn lower_and_infer_borrows(
    module: &ori_ir::ast::Module,
    function_sigs: &[ori_types::FunctionSig],
    canon: &ori_ir::canon::CanonResult,
    interner: &ori_ir::StringInterner,
    pool: &ori_types::Pool,
    impl_sigs: &[(Name, ori_types::FunctionSig)],
    imported_functions: &[ori_llvm::evaluator::ImportedFunctionForCodegen<'_>],
    mono_instances: &[ori_types::MonoInstance],
    user_types: &[ori_types::TypeEntry],
) -> ArcLoweringResult {
    let classifier = ori_arc::ArcClassifier::new(pool);
    let mut local_lowered: Vec<(ori_arc::ArcFunction, Vec<ori_arc::ArcFunction>)> = Vec::new();
    let mut imported_lowered: Vec<(ori_arc::ArcFunction, Vec<ori_arc::ArcFunction>)> = Vec::new();
    let mut arc_problems = Vec::new();

    // Lower module functions (local — uses main pool)
    for (func, sig) in module.functions.iter().zip(function_sigs.iter()) {
        if sig.is_generic() {
            continue;
        }
        let (arc_fn, lambdas) = crate::arc_lowering::lower_to_arc(
            func.name,
            sig,
            func.name,
            canon,
            interner,
            pool,
            &mut arc_problems,
            None,
        );
        local_lowered.push((arc_fn, lambdas));
    }

    // Lower imported functions using the main pool. All Idx values in
    // imp_fn.sig and imp_fn.canon have been re-interned into the main pool
    // by the caller, so we can safely use the same pool and classifier.
    let builtins = ori_arc::BuiltinOwnershipSets::new(interner);
    let mut imported_sigs: FxHashMap<Name, ori_arc::AnnotatedSig> = FxHashMap::default();
    for imp_fn in imported_functions {
        if imp_fn.sig.is_generic() {
            continue;
        }
        let (arc_fn, lambdas) = crate::arc_lowering::lower_to_arc(
            imp_fn.function.name,
            &imp_fn.sig,
            imp_fn.function.name,
            imp_fn.canon,
            interner,
            pool,
            &mut arc_problems,
            None,
        );
        // Borrow inference uses the main pool's classifier — all types are
        // in the same pool after re-interning.
        let imp_flat: Vec<ori_arc::ArcFunction> = std::iter::once(&arc_fn)
            .chain(lambdas.iter())
            .cloned()
            .collect();
        let imp_borrow_sigs = ori_arc::infer_borrows_scc(&imp_flat, &classifier, &builtins);
        imported_sigs.extend(imp_borrow_sigs);
        imported_lowered.push((arc_fn, lambdas));
    }

    // Lower impl method functions (local — uses main pool).
    // Use type-qualified names from the impl block's self-type (not
    // sig.param_types[0]) to prevent collisions (TPR-03-043/045/047).
    {
        let mut sig_iter = impl_sigs.iter();
        for impl_def in &module.impls {
            let type_name_name = impl_def.self_path.first().copied();
            let self_type_idx = type_name_name.and_then(|name| {
                user_types
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
                let qualified_name = if let Some(idx) = self_type_idx {
                    let method_str = interner.lookup(method.name);
                    interner.intern(&format!("__impl_{}_{method_str}", idx.raw()))
                } else {
                    method.name
                };
                let (arc_fn, lambdas) = if let Some(tn) = type_name_name {
                    crate::arc_lowering::lower_impl_method_to_arc(
                        qualified_name,
                        sig,
                        method.name,
                        tn,
                        canon,
                        interner,
                        pool,
                        &mut arc_problems,
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
                        &mut arc_problems,
                        None,
                    )
                };
                local_lowered.push((arc_fn, lambdas));
            }
            // Skip default trait methods in sig_iter
            if let Some(trait_path) = &impl_def.trait_path {
                if let Some(&trait_name) = trait_path.last() {
                    let overridden: rustc_hash::FxHashSet<Name> =
                        impl_def.methods.iter().map(|m| m.name).collect();
                    if let Some(trait_def) = module.traits.iter().find(|t| t.name == trait_name) {
                        for item in &trait_def.items {
                            if let ori_ir::TraitItem::DefaultMethod(default) = item {
                                if !overridden.contains(&default.name) {
                                    let Some((_, sig)) = sig_iter.next() else {
                                        break;
                                    };
                                    if sig.is_generic() {
                                        continue;
                                    }
                                    let qualified_name = if let Some(idx) = self_type_idx {
                                        let method_str = interner.lookup(default.name);
                                        interner
                                            .intern(&format!("__impl_{}_{method_str}", idx.raw()))
                                    } else {
                                        default.name
                                    };
                                    let (arc_fn, lambdas) = if let Some(tn) = type_name_name {
                                        crate::arc_lowering::lower_impl_method_to_arc(
                                            qualified_name,
                                            sig,
                                            default.name,
                                            tn,
                                            canon,
                                            interner,
                                            pool,
                                            &mut arc_problems,
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
                                            &mut arc_problems,
                                            None,
                                        )
                                    };
                                    local_lowered.push((arc_fn, lambdas));
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // Lower monomorphized generic functions (local — uses main pool)
    let mono_functions = ori_llvm::monomorphize::collect_mono_functions(
        mono_instances,
        function_sigs,
        interner,
        pool,
    );
    for mono_fn in &mono_functions {
        let (arc_fn, lambdas) = crate::arc_lowering::lower_to_arc(
            mono_fn.mangled_name,
            &mono_fn.sig,
            mono_fn.original_name,
            canon,
            interner,
            pool,
            &mut arc_problems,
            Some(&mono_fn.body_type_map),
        );
        local_lowered.push((arc_fn, lambdas));
    }

    // Check for ARC lowering problems (unsupported patterns, internal errors).
    // Mirrors the check in compile_common.rs — without this, the JIT runner
    // silently swallows lowering errors that the AOT path would report.
    if !arc_problems.is_empty() {
        use crate::problem::codegen::{emit_codegen_diagnostics, CodegenDiagnostics};
        let mut acc = CodegenDiagnostics::new();
        acc.add_arc_problems(&arc_problems);
        if emit_codegen_diagnostics(acc) {
            return (FxHashMap::default(), FxHashMap::default());
        }
    }

    // Borrow inference for local functions using the main pool's classifier.
    // Imported function sigs are already computed above — if local functions
    // call imported functions but don't find their sigs in the call graph,
    // that's safe: unknown callees are treated conservatively (params stay
    // Borrowed), and RC emission uses the merged annotated_sigs map to insert
    // correct inc/dec at call sites regardless.
    let local_flat: Vec<ori_arc::ArcFunction> = local_lowered
        .iter()
        .flat_map(|(f, lambdas)| std::iter::once(f).chain(lambdas.iter()))
        .cloned()
        .collect();

    let mut annotated_sigs = ori_arc::infer_borrows_scc(&local_flat, &classifier, &builtins);
    annotated_sigs.extend(imported_sigs);

    // Build cache: Name → (ArcFunction, Vec<ArcFunction>)
    let total = local_lowered.len() + imported_lowered.len();
    let mut arc_cache: FxHashMap<Name, (ori_arc::ArcFunction, Vec<ori_arc::ArcFunction>)> =
        FxHashMap::with_capacity_and_hasher(total, rustc_hash::FxBuildHasher);
    for (arc_fn, lambdas) in local_lowered.into_iter().chain(imported_lowered) {
        arc_cache.insert(arc_fn.name, (arc_fn, lambdas));
    }

    (annotated_sigs, arc_cache)
}
