//! Backend-neutral ARC batch assembly for the LLVM JIT test runner.
//!
//! Lowers every local, imported, impl, monomorphized, lambda, and test body before
//! executable realization. LLVM receives the resulting closed artifact and owns
//! no AIMS or borrow-analysis policy.

use rustc_hash::{FxHashMap, FxHashSet};

use ori_ir::Name;

/// Complete pre-AIMS JIT body inventory plus exact impl identities.
pub(crate) struct JitArcLowering {
    pub(crate) prepared_batch: crate::realization::PreparedArcBatch,
    /// The exact checked specialization inventory used for target rewriting.
    pub(crate) mono_functions: Vec<ori_repr::monomorphize::MonoFunction>,
    pub(crate) user_drop_bindings: Vec<ori_repr::executable::UserDropBinding>,
    pub(crate) impl_emission_names: Vec<Option<Name>>,
    pub(crate) reachable_imports: FxHashSet<Name>,
}

// The prepared executable batch owns its own diagnostics. Report the exact JIT
// specialization and ownership-metadata inventory retained alongside it.
impl std::fmt::Debug for JitArcLowering {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("JitArcLowering")
            .field("mono_function_count", &self.mono_functions.len())
            .field("user_drop_binding_count", &self.user_drop_bindings.len())
            .field("impl_emission_names", &self.impl_emission_names)
            .finish_non_exhaustive()
    }
}

/// Typed failure while lowering and preparing one JIT executable unit.
#[derive(Debug, thiserror::Error)]
pub(crate) enum JitArcLoweringError {
    /// Raw declarations could not form one semantic callable seed inventory.
    #[error(transparent)]
    CallableCensus(#[from] crate::realization::CallableCensusError),
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
    /// Generic targets exposed by specialized lambda bodies could not be closed.
    #[error(transparent)]
    GenericMonoClosure(#[from] crate::realization::GenericMonoClosureError),
}

pub(crate) struct JitArcLoweringInput<'a, 'test, 'import> {
    pub(crate) parse: &'a crate::parser::ParseOutput,
    pub(crate) typed: &'a ori_types::TypeCheckResult,
    pub(crate) tests: &'a [&'test ori_ir::TestDef],
    pub(crate) function_sigs: &'a [ori_types::FunctionSig],
    pub(crate) canon: &'a ori_ir::canon::CanonResult,
    pub(crate) interner: &'a ori_ir::StringInterner,
    pub(crate) pool: &'a mut ori_types::Pool,
    pub(crate) import_sigs: &'a [ori_repr::monomorphize::ImportSig],
    pub(crate) imported_functions: &'a [ori_llvm::evaluator::ImportedFunctionForCodegen<'import>],
    pub(crate) imported_mono_fns: &'a [crate::commands::ImportedMonoFn],
    pub(crate) imported_generic_templates: &'a [crate::realization::ImportedGenericTemplate],
    pub(crate) re_interned_canons: &'a [ori_ir::canon::CanonResult],
    pub(crate) imported_function_modules: &'a [usize],
    pub(crate) imported_target_maps: &'a [FxHashMap<Name, Name>],
    pub(crate) root_import_targets: &'a FxHashMap<Name, Name>,
}

// Imported bodies and shared compiler owners keep their own diagnostic
// surfaces. Report the input inventory dimensions consumed by JIT lowering.
impl std::fmt::Debug for JitArcLoweringInput<'_, '_, '_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("JitArcLoweringInput")
            .field("test_count", &self.tests.len())
            .field("function_signature_count", &self.function_sigs.len())
            .field("import_signature_count", &self.import_sigs.len())
            .field("imported_function_count", &self.imported_functions.len())
            .field("imported_mono_count", &self.imported_mono_fns.len())
            .field("re_interned_canon_count", &self.re_interned_canons.len())
            .field(
                "imported_target_map_count",
                &self.imported_target_maps.len(),
            )
            .finish_non_exhaustive()
    }
}

type LoweredBody = (ori_arc::ArcFunction, Vec<ori_arc::ArcFunction>);

struct SpecializedBodies {
    impl_groups: Vec<crate::realization::ArcFunctionGroup>,
    derived_groups: Vec<crate::realization::ArcFunctionGroup>,
    impl_targets: FxHashMap<(ori_types::Idx, Name), Name>,
    user_drop_bindings: Vec<ori_repr::executable::UserDropBinding>,
    impl_emission_names: Vec<Option<Name>>,
    mono_groups: Vec<crate::realization::ArcFunctionGroup>,
    mono_inventory: crate::realization::MonoFunctionInventory,
}

fn lower_local_bodies(
    input: &JitArcLoweringInput<'_, '_, '_>,
    problems: &mut Vec<ori_arc::ArcProblem>,
) -> Result<Vec<LoweredBody>, JitArcLoweringError> {
    let seeds = crate::realization::CallableCensusBuilder::new(input.interner)
        .source_functions(&input.parse.module.functions, input.function_sigs)?;
    let mut bodies = Vec::new();
    for seed in seeds {
        if seed.signature.requires_specialization() {
            continue;
        }
        let mut context = crate::arc_lowering::ArcLoweringContext {
            canon: input.canon,
            interner: input.interner,
            pool: &*input.pool,
            problems,
        };
        let lowered = crate::arc_lowering::lower_to_arc(
            seed.function.name,
            seed.signature,
            seed.function.name,
            &mut context,
            None,
        );
        bodies.push(super::imported_call_closure::rewrite_lowered_body(
            lowered,
            input.root_import_targets,
            false,
        ));
    }
    Ok(bodies)
}

fn lower_imported_bodies(
    input: &JitArcLoweringInput<'_, '_, '_>,
    problems: &mut Vec<ori_arc::ArcProblem>,
) -> Vec<LoweredBody> {
    let mut bodies = Vec::new();
    for (imported_index, imported) in input.imported_functions.iter().enumerate() {
        if imported.sig.requires_specialization() {
            continue;
        }
        let mut context = crate::arc_lowering::ArcLoweringContext {
            canon: imported.canon,
            interner: input.interner,
            pool: &*input.pool,
            problems,
        };
        let lowered = crate::arc_lowering::lower_to_arc(
            imported.function.name,
            &imported.sig,
            imported.function.name,
            &mut context,
            None,
        );
        let module_index = input.imported_function_modules[imported_index];
        bodies.push(super::imported_call_closure::rewrite_lowered_body(
            lowered,
            &input.imported_target_maps[module_index],
            true,
        ));
    }
    bodies
}

fn lower_imported_mono_bodies(
    input: &JitArcLoweringInput<'_, '_, '_>,
    problems: &mut Vec<ori_arc::ArcProblem>,
) -> Vec<LoweredBody> {
    let mut bodies = Vec::new();
    for imported in input.imported_mono_fns {
        let mono = &imported.function;
        let mut context = crate::arc_lowering::ArcLoweringContext {
            canon: &input.re_interned_canons[imported.module_index],
            interner: input.interner,
            pool: &*input.pool,
            problems,
        };
        let lowered = match imported.body {
            crate::commands::ImportedMonoBody::Function(source_name) => {
                crate::arc_lowering::lower_to_arc(
                    mono.mangled_name,
                    &mono.sig,
                    source_name,
                    &mut context,
                    Some(&mono.body_type_map),
                )
            }
            crate::commands::ImportedMonoBody::ImplMethod(source_body) => {
                crate::arc_lowering::lower_impl_method_to_arc_by_source(
                    mono.mangled_name,
                    &mono.sig,
                    source_body,
                    &mut context,
                    Some(&mono.body_type_map),
                )
            }
        };
        bodies.push(super::imported_call_closure::rewrite_lowered_body(
            lowered,
            &input.imported_target_maps[imported.module_index],
            true,
        ));
    }
    bodies
}

fn lower_test_bodies(
    input: &JitArcLoweringInput<'_, '_, '_>,
    problems: &mut Vec<ori_arc::ArcProblem>,
) -> Vec<LoweredBody> {
    input
        .tests
        .iter()
        .map(|test| {
            let body = input.canon.root_for(test.name).unwrap_or(input.canon.root);
            let lowered = ori_arc::lower_function_can(
                ori_arc::ArcLoweringInput {
                    name: test.name,
                    params: &[],
                    return_type: ori_types::Idx::UNIT,
                    body,
                    canon: input.canon,
                    interner: input.interner,
                    pool: &*input.pool,
                    type_subst: None,
                    const_bindings: None,
                    is_fbip: false,
                },
                problems,
            );
            super::imported_call_closure::rewrite_lowered_body(
                lowered,
                input.root_import_targets,
                false,
            )
        })
        .collect()
}

fn lower_specialized_bodies(
    input: &JitArcLoweringInput<'_, '_, '_>,
    problems: &mut Vec<ori_arc::ArcProblem>,
) -> Result<SpecializedBodies, JitArcLoweringError> {
    let crate::realization::ImplMethodAnalysis {
        groups: impl_groups,
        targets: mut impl_targets,
        user_drop_bindings,
        emission_names: impl_emission_names,
        ..
    } = match crate::realization::lower_impl_methods_for_analysis(
        input.parse,
        input.typed,
        input.interner,
        input.canon,
        &*input.pool,
    ) {
        Ok(analysis) => analysis,
        Err(found) => {
            problems.extend(found);
            crate::realization::ImplMethodAnalysis {
                groups: Vec::new(),
                targets: FxHashMap::default(),
                user_drop_bindings: Vec::new(),
                emission_names: Vec::new(),
            }
        }
    };
    let derived = match crate::realization::lower_non_generic_derived_methods_for_analysis(
        &input.typed.typed.accepted_derives,
        &input.typed.typed.derived_call_plans,
        input.interner,
        &*input.pool,
    ) {
        Ok(analysis) => analysis,
        Err(found) => {
            problems.extend(found);
            crate::realization::DerivedMethodAnalysis {
                groups: Vec::new(),
                targets: FxHashMap::default(),
            }
        }
    };
    for (key, target) in derived.targets {
        impl_targets.entry(key).or_insert(target);
    }
    let mono_functions = ori_repr::monomorphize::collect_mono_functions(
        &input.typed.typed.mono_instances,
        input.function_sigs,
        &input.typed.typed.impl_sigs,
        &input.typed.typed.accepted_derives,
        input.import_sigs,
        input.interner,
        &*input.pool,
    );
    let mut mono_context = crate::arc_lowering::ArcLoweringContext {
        canon: input.canon,
        interner: input.interner,
        pool: &*input.pool,
        problems,
    };
    let mono_groups = crate::realization::lower_mono_functions_for_analysis(
        &mono_functions,
        &input.typed.typed.accepted_derives,
        &input.typed.typed.derived_call_plans,
        &mut mono_context,
    );
    let mono_inventory = crate::realization::MonoFunctionInventory::try_new(
        mono_functions,
        input
            .imported_mono_fns
            .iter()
            .map(|imported| imported.function.clone()),
        input.interner,
    )?;
    if let Err(found) = crate::realization::extend_mono_method_targets(
        &mut impl_targets,
        mono_inventory.all(),
        input.interner,
        &*input.pool,
    ) {
        problems.extend(found);
    }
    Ok(SpecializedBodies {
        impl_groups,
        derived_groups: derived.groups,
        impl_targets,
        user_drop_bindings,
        impl_emission_names,
        mono_groups,
        mono_inventory,
    })
}

/// Lower every body in one JIT executable unit to ARC IR.
///
/// The returned cache is pre-specialized and has mono/operator/impl targets
/// rewritten before the shared whole-program realization runs.
///
/// Functions lowered: module functions, imported functions, impl methods,
/// monomorphized generic functions, test bodies, and every nested lambda.
/// Every JIT body source lowered to ARC IR, before generic-mono closure.
struct LoweredJitBodies {
    local_lowered: Vec<LoweredBody>,
    imported_lowered: Vec<LoweredBody>,
    imported_mono_lowered: Vec<LoweredBody>,
    impl_groups: Vec<crate::realization::ArcFunctionGroup>,
    derived_groups: Vec<crate::realization::ArcFunctionGroup>,
    impl_targets: FxHashMap<(ori_types::Idx, Name), Name>,
    user_drop_bindings: Vec<ori_repr::executable::UserDropBinding>,
    impl_emission_names: Vec<Option<Name>>,
    mono_inventory: crate::realization::MonoFunctionInventory,
    arc_problems: Vec<ori_arc::ArcProblem>,
}

/// Lower local, imported, imported-mono, specialized, and test bodies to ARC IR.
fn lower_every_jit_body_source(
    input: &JitArcLoweringInput<'_, '_, '_>,
) -> Result<LoweredJitBodies, JitArcLoweringError> {
    let mut arc_problems = Vec::new();
    let mut local_lowered = lower_local_bodies(input, &mut arc_problems)?;
    let imported_lowered = lower_imported_bodies(input, &mut arc_problems);
    let imported_mono_lowered = lower_imported_mono_bodies(input, &mut arc_problems);
    let SpecializedBodies {
        impl_groups,
        derived_groups,
        impl_targets,
        user_drop_bindings,
        impl_emission_names,
        mono_groups,
        mono_inventory,
    } = lower_specialized_bodies(input, &mut arc_problems)?;
    for group in mono_groups {
        local_lowered.push(
            super::imported_call_closure::rewrite_group(group, input.root_import_targets, true)
                .into_parts(),
        );
    }
    local_lowered.extend(lower_test_bodies(input, &mut arc_problems));
    Ok(LoweredJitBodies {
        local_lowered,
        imported_lowered,
        imported_mono_lowered,
        impl_groups,
        derived_groups,
        impl_targets,
        user_drop_bindings,
        impl_emission_names,
        mono_inventory,
        arc_problems,
    })
}

/// Flatten every lowered body source into one root group list, rewriting impl
/// and derived groups against the root import-target map, then close the
/// reachable-import set over that root list.
fn assemble_root_groups(
    local_lowered: Vec<LoweredBody>,
    impl_groups: Vec<crate::realization::ArcFunctionGroup>,
    derived_groups: Vec<crate::realization::ArcFunctionGroup>,
    imported_mono_lowered: Vec<LoweredBody>,
    imported_lowered: Vec<LoweredBody>,
    root_import_targets: &FxHashMap<Name, Name>,
) -> (Vec<crate::realization::ArcFunctionGroup>, FxHashSet<Name>) {
    let mut root_groups: Vec<_> = local_lowered
        .into_iter()
        .map(crate::realization::ArcFunctionGroup::from)
        .chain(impl_groups.into_iter().map(|group| {
            super::imported_call_closure::rewrite_group(group, root_import_targets, false)
        }))
        .chain(derived_groups.into_iter().map(|group| {
            super::imported_call_closure::rewrite_group(group, root_import_targets, false)
        }))
        .chain(
            imported_mono_lowered
                .into_iter()
                .map(crate::realization::ArcFunctionGroup::from),
        )
        .collect();
    let imported_candidates = imported_lowered
        .into_iter()
        .map(crate::realization::ArcFunctionGroup::from)
        .collect();
    let (reachable_imported_groups, reachable_imports) =
        super::imported_call_closure::close_reachable_imports(&root_groups, imported_candidates);
    root_groups.extend(reachable_imported_groups);
    (root_groups, reachable_imports)
}

/// Rewrite every closed group whose mangled name names an imported generic
/// specialization against that import's own target map.
fn rewrite_closed_groups_for_imports(
    closed_groups: Vec<crate::realization::ArcFunctionGroup>,
    mono_functions: &[ori_repr::monomorphize::MonoFunction],
    imported_generic_templates: &[crate::realization::ImportedGenericTemplate],
    imported_target_maps: &[FxHashMap<Name, Name>],
) -> Vec<crate::realization::ArcFunctionGroup> {
    let imported_modules_by_mono: FxHashMap<_, _> = mono_functions
        .iter()
        .filter(|mono| mono.is_imported)
        .filter_map(|mono| {
            imported_generic_templates
                .iter()
                .find(|template| template.local_name == mono.identity.original_name())
                .map(|template| (mono.mangled_name, template.module_index))
        })
        .collect();
    closed_groups
        .into_iter()
        .map(|group| {
            let Some(&module_index) = imported_modules_by_mono.get(&group.parent_name()) else {
                return group;
            };
            super::imported_call_closure::rewrite_group(
                group,
                &imported_target_maps[module_index],
                true,
            )
        })
        .collect()
}

pub(crate) fn lower_jit_arc_program(
    input: JitArcLoweringInput<'_, '_, '_>,
) -> Result<JitArcLowering, JitArcLoweringError> {
    let LoweredJitBodies {
        local_lowered,
        imported_lowered,
        imported_mono_lowered,
        impl_groups,
        derived_groups,
        mut impl_targets,
        user_drop_bindings,
        impl_emission_names,
        mono_inventory,
        arc_problems,
    } = lower_every_jit_body_source(&input)?;
    let interner = input.interner;
    let pool = input.pool;
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

    let (mut groups, reachable_imports) = assemble_root_groups(
        local_lowered,
        impl_groups,
        derived_groups,
        imported_mono_lowered,
        imported_lowered,
        input.root_import_targets,
    );

    crate::realization::CallableCensusBuilder::new(interner)
        .close_builtin_targets(&mut groups, pool)?;

    let mono_functions = mono_inventory.into_all();
    let local_generic_type_params =
        crate::realization::generic_type_param_map(&input.typed.typed.types);
    let closed = crate::realization::close_generic_mono_targets(
        crate::realization::GenericMonoClosureInput {
            groups,
            mono_functions,
            mono_instances: &input.typed.typed.mono_instances,
            function_sigs: input.function_sigs,
            local_generic_type_params: &local_generic_type_params,
            impl_sigs: &input.typed.typed.impl_sigs,
            accepted_derives: &input.typed.typed.accepted_derives,
            derived_call_plans: &input.typed.typed.derived_call_plans,
            import_sigs: input.import_sigs,
            imported_generic_templates: input.imported_generic_templates,
            re_interned_canons: input.re_interned_canons,
            canon: input.canon,
            interner,
            pool,
        },
    )?;
    crate::realization::extend_mono_method_targets(
        &mut impl_targets,
        &closed.mono_functions,
        interner,
        pool,
    )
    .map_err(|problems| JitArcLoweringError::ArcLowering {
        count: problems.len(),
        problems,
    })?;
    let closed_groups = rewrite_closed_groups_for_imports(
        closed.groups,
        &closed.mono_functions,
        input.imported_generic_templates,
        input.imported_target_maps,
    );
    let prepared_batch = crate::realization::LoweredArcBatch::try_from_groups(
        closed_groups,
        interner,
    )?
    .prepare(&closed.mono_functions, &impl_targets, pool, interner)?;
    let mono_functions = closed.mono_functions;

    Ok(JitArcLowering {
        prepared_batch,
        mono_functions,
        user_drop_bindings,
        impl_emission_names,
        reachable_imports,
    })
}
