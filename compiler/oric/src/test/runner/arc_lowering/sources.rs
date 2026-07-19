//! ARC lowering for each local, imported, specialized, and test body source.

use rustc_hash::FxHashMap;

use ori_ir::Name;

use super::super::imported_call_closure;
use super::{JitArcLoweringError, JitArcLoweringInput};

pub(super) type LoweredBody = (ori_arc::ArcFunction, Vec<ori_arc::ArcFunction>);

struct SpecializedBodies {
    impl_groups: Vec<crate::realization::ArcFunctionGroup>,
    derived_groups: Vec<crate::realization::ArcFunctionGroup>,
    impl_targets: FxHashMap<(ori_types::Idx, Name), Name>,
    impl_producer_targets: FxHashMap<ori_types::MethodProducer, Name>,
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
        bodies.push(imported_call_closure::rewrite_lowered_body(
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
        bodies.push(imported_call_closure::rewrite_lowered_body(
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
        bodies.push(imported_call_closure::rewrite_lowered_body(
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
            imported_call_closure::rewrite_lowered_body(lowered, input.root_import_targets, false)
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
        producer_targets: impl_producer_targets,
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
                producer_targets: FxHashMap::default(),
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
        impl_producer_targets,
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
pub(super) struct LoweredJitBodies {
    pub(super) local_lowered: Vec<LoweredBody>,
    pub(super) imported_lowered: Vec<LoweredBody>,
    pub(super) imported_mono_lowered: Vec<LoweredBody>,
    pub(super) impl_groups: Vec<crate::realization::ArcFunctionGroup>,
    pub(super) derived_groups: Vec<crate::realization::ArcFunctionGroup>,
    pub(super) impl_targets: FxHashMap<(ori_types::Idx, Name), Name>,
    pub(super) impl_producer_targets: FxHashMap<ori_types::MethodProducer, Name>,
    pub(super) user_drop_bindings: Vec<ori_repr::executable::UserDropBinding>,
    pub(super) impl_emission_names: Vec<Option<Name>>,
    pub(super) mono_inventory: crate::realization::MonoFunctionInventory,
    pub(super) arc_problems: Vec<ori_arc::ArcProblem>,
}

/// Lower local, imported, imported-mono, specialized, and test bodies to ARC IR.
pub(super) fn lower_every_jit_body_source(
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
        impl_producer_targets,
        user_drop_bindings,
        impl_emission_names,
        mono_groups,
        mono_inventory,
    } = lower_specialized_bodies(input, &mut arc_problems)?;
    for group in mono_groups {
        local_lowered.push(
            imported_call_closure::rewrite_group(group, input.root_import_targets, true)
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
        impl_producer_targets,
        user_drop_bindings,
        impl_emission_names,
        mono_inventory,
        arc_problems,
    })
}
