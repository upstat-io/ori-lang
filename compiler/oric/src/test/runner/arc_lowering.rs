//! Backend-neutral ARC batch assembly for the LLVM JIT test runner.
//!
//! Lowers every local, imported, impl, monomorphized, lambda, and test body before
//! executable realization. LLVM receives the resulting closed artifact and owns
//! no AIMS or borrow-analysis policy.

use rustc_hash::FxHashMap;

use ori_ir::Name;

/// Complete pre-AIMS JIT body inventory plus exact impl identities.
pub(crate) struct JitArcLowering {
    pub(crate) prepared_batch: crate::realization::PreparedArcBatch,
    /// The exact checked specialization inventory used for target rewriting.
    pub(crate) mono_functions: Vec<ori_repr::monomorphize::MonoFunction>,
    pub(crate) user_drop_bindings: Vec<ori_repr::executable::UserDropBinding>,
    pub(crate) impl_emission_names: Vec<Option<Name>>,
}

/// Typed failure while lowering and preparing one JIT executable unit.
#[derive(Debug, thiserror::Error)]
pub(crate) enum JitArcLoweringError {
    /// Canonical-to-ARC lowering produced invalid bodies.
    #[error(
        "ARC lowering produced {count} problem(s): {problems:?}. Fix the reported Ori source errors; if no source error is shown, report this complete message"
    )]
    ArcLowering {
        count: usize,
        problems: Vec<ori_arc::ArcProblem>,
    },
    /// The shared lowered-to-prepared ARC seam rejected the batch.
    #[error(transparent)]
    Preparation(#[from] crate::realization::ArcBatchPreparationError),
    /// Final mono identities could not be bound to one source namespace.
    #[error(transparent)]
    MonoInventory(#[from] crate::realization::MonoFunctionInventoryError),
}

/// Lower every body in one JIT executable unit to ARC IR.
///
/// The returned cache is pre-specialized and has mono/operator/impl targets
/// rewritten before the shared whole-program realization runs.
///
/// Functions lowered: module functions, imported functions, impl methods,
/// monomorphized generic functions, test bodies, and every nested lambda.
#[expect(
    clippy::too_many_arguments,
    reason = "mirrors the data flow from run_file_llvm — all inputs are required"
)]
#[expect(
    clippy::too_many_lines,
    reason = "ARC lowering pipeline — local, imported, impl, and mono functions in sequence"
)]
pub(crate) fn lower_jit_arc_program(
    parse_result: &crate::parser::ParseOutput,
    type_result: &ori_types::TypeCheckResult,
    tests: &[&ori_ir::TestDef],
    function_sigs: &[ori_types::FunctionSig],
    canon: &ori_ir::canon::CanonResult,
    interner: &ori_ir::StringInterner,
    pool: &mut ori_types::Pool,
    import_sigs: &[ori_repr::monomorphize::ImportSig],
    imported_functions: &[ori_llvm::evaluator::ImportedFunctionForCodegen<'_>],
    imported_mono_fns: &[(ori_repr::monomorphize::MonoFunction, usize, Name)],
    re_interned_canons: &[ori_ir::canon::CanonResult],
) -> Result<JitArcLowering, JitArcLoweringError> {
    let module = &parse_result.module;
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
    // imp_fn.sig and imp_fn.canon were re-interned into the main pool by the
    // caller, so the same pool applies.
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
        imported_lowered.push((arc_fn, lambdas));
    }

    // Lower imported monomorphized generic functions with their module's canon.
    for (mono_fn, canon_idx, source_body_name) in imported_mono_fns {
        let (arc_fn, lambdas) = crate::arc_lowering::lower_to_arc(
            mono_fn.mangled_name,
            &mono_fn.sig,
            *source_body_name, // Use SOURCE name for canon.root_for() lookup
            &re_interned_canons[*canon_idx],
            interner,
            pool,
            &mut arc_problems,
            Some(&mono_fn.body_type_map),
        );
        imported_lowered.push((arc_fn, lambdas));
    }

    // Impl lowering and qualified target identity are centralized with AOT.
    let crate::realization::ImplMethodAnalysis {
        groups: impl_groups,
        targets: mut impl_targets,
        user_drop_bindings,
        emission_names: impl_emission_names,
        ..
    } = match crate::realization::lower_impl_methods_for_analysis(
        parse_result,
        type_result,
        interner,
        canon,
        pool,
    ) {
        Ok(analysis) => analysis,
        Err(problems) => {
            arc_problems.extend(problems);
            crate::realization::ImplMethodAnalysis {
                groups: Vec::new(),
                targets: FxHashMap::default(),
                user_drop_bindings: Vec::new(),
                emission_names: Vec::new(),
            }
        }
    };
    let derived = match crate::realization::lower_non_generic_derived_methods_for_analysis(
        &type_result.typed.accepted_derives,
        interner,
        pool,
    ) {
        Ok(analysis) => analysis,
        Err(problems) => {
            arc_problems.extend(problems);
            crate::realization::DerivedMethodAnalysis {
                groups: Vec::new(),
                targets: FxHashMap::default(),
            }
        }
    };
    for (key, target) in derived.targets {
        impl_targets.entry(key).or_insert(target);
    }
    // Lower monomorphized generic functions (local — uses main pool)
    let mono_functions = ori_repr::monomorphize::collect_mono_functions(
        &type_result.typed.mono_instances,
        function_sigs,
        &type_result.typed.impl_sigs,
        &type_result.typed.accepted_derives,
        import_sigs,
        interner,
        pool,
    );
    let mono_inventory = crate::realization::MonoFunctionInventory::try_new(
        mono_functions,
        imported_mono_fns
            .iter()
            .map(|(function, _, _)| function.clone()),
        interner,
    )?;
    for mono_fn in mono_inventory.local_bodies() {
        if let Some(group) = crate::realization::lower_mono_function_for_analysis(
            mono_fn,
            &type_result.typed.accepted_derives,
            canon,
            interner,
            pool,
            &mut arc_problems,
        ) {
            local_lowered.push(group.into_parts());
        }
    }

    // Tests are ordinary executable roots. Lower them before AIMS so the JIT
    // wrapper projects an already-realized body instead of creating a private
    // backend-local analysis island.
    for test in tests {
        let body = canon.root_for(test.name).unwrap_or(canon.root);
        local_lowered.push(ori_arc::lower_function_can(
            test.name,
            &[],
            ori_types::Idx::UNIT,
            body,
            canon,
            interner,
            pool,
            &mut arc_problems,
            false,
            None,
        ));
    }

    // INVARIANT: None = ARC lowering errored (a compile failure) — distinct
    // from Some(empty) (nothing to compile); an empty arc_cache recurses in
    // codegen resolving missing callees.
    if !arc_problems.is_empty() {
        use crate::problem::codegen::{emit_codegen_diagnostics, CodegenDiagnostics};
        let mut acc = CodegenDiagnostics::new();
        acc.add_arc_problems(&arc_problems);
        if emit_codegen_diagnostics(acc) {
            return Err(JitArcLoweringError::ArcLowering {
                count: arc_problems.len(),
                problems: arc_problems,
            });
        }
    }

    let groups = local_lowered
        .into_iter()
        .map(crate::realization::ArcFunctionGroup::from)
        .chain(impl_groups)
        .chain(derived.groups)
        .chain(
            imported_lowered
                .into_iter()
                .map(crate::realization::ArcFunctionGroup::from),
        );

    let prepared_batch = crate::realization::LoweredArcBatch::try_from_groups(groups, interner)?
        .prepare(mono_inventory.all(), &impl_targets, pool, interner)?;
    let mono_functions = mono_inventory.into_all();

    Ok(JitArcLowering {
        prepared_batch,
        mono_functions,
        user_drop_bindings,
        impl_emission_names,
    })
}
