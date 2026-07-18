//! Complete pre-AIMS ARC lowering for the codegen pipeline.
//!
//! Lowers every function (local, mono, imported mono) to ARC IR. The resulting
//! batch is consumed once by backend-neutral executable realization; LLVM does
//! not run a second ownership calculus over these bodies.

use ori_ir::canon::CanonResult;
use ori_types::{FunctionSig, Pool};
use oric::ir::StringInterner;
use oric::parser::ParseOutput;
use rustc_hash::FxHashSet;

use super::imported_mono::ImportedSurfaces;

/// Complete pre-AIMS ARC families and their monomorphized emission inventory.
///
pub(super) struct ArcBatchLoweringResult {
    /// Pre-lowered ARC parent/lambda families before shared preparation.
    pub(super) groups: Vec<crate::realization::ArcFunctionGroup>,
    /// Monomorphized generic functions (reused by codegen to avoid recomputation).
    pub(super) mono_functions: Vec<ori_repr::monomorphize::MonoFunction>,
}

/// Failure before closed executable realization can consume the ARC batch.
pub(super) enum ArcBatchLoweringFailure {
    /// Raw declarations could not form one semantic callable seed inventory.
    CallableCensus(crate::realization::CallableCensusError),
    /// ARC lowering emitted its structured diagnostics.
    ArcLowering,
    /// Final mono identities could not be bound to one source namespace.
    MonoInventory(crate::realization::MonoFunctionInventoryError),
}

#[derive(Clone, Copy)]
pub(super) struct ArcBatchLoweringInput<'a> {
    pub(super) parse: &'a ParseOutput,
    pub(super) function_sigs: &'a [FunctionSig],
    pub(super) impl_sigs: &'a [ori_types::ImplSig],
    pub(super) import_sigs: &'a [ori_repr::monomorphize::ImportSig],
    pub(super) imported: ImportedSurfaces<'a>,
    pub(super) canon: &'a CanonResult,
    pub(super) interner: &'a StringInterner,
    pub(super) pool: &'a Pool,
    pub(super) mono_instances: &'a [ori_types::MonoInstance],
    pub(super) accepted_derives: &'a [ori_types::AcceptedDerivedImpl],
    pub(super) derived_call_plans: &'a [ori_types::DerivedCallPlan],
}

fn lower_source_groups(
    input: &ArcBatchLoweringInput<'_>,
    problems: &mut Vec<ori_arc::ArcProblem>,
    exempt: &FxHashSet<u32>,
) -> Result<Vec<crate::realization::ArcFunctionGroup>, ArcBatchLoweringFailure> {
    let seeds = crate::realization::CallableCensusBuilder::new(input.interner)
        .source_functions(&input.parse.module.functions, input.function_sigs)
        .map_err(ArcBatchLoweringFailure::CallableCensus)?;
    let mut groups = Vec::new();
    for seed in seeds {
        if seed.signature.is_generic() {
            continue;
        }
        let mut context = crate::arc_lowering::ArcLoweringContext {
            canon: input.canon,
            interner: input.interner,
            pool: input.pool,
            problems,
        };
        let (function, lambdas) = crate::arc_lowering::lower_to_arc(
            seed.function.name,
            seed.signature,
            seed.function.name,
            &mut context,
            None,
        );
        super::pc2_hooks::run_pc2_hook_aot(
            input.pool,
            &function,
            &lambdas,
            input.interner,
            exempt,
            "aot_pre_mono",
            "aot_pre_mono_lambda",
        );
        groups.push(crate::realization::ArcFunctionGroup::new(function, lambdas));
    }
    Ok(groups)
}

fn lower_imported_mono_groups(
    input: &ArcBatchLoweringInput<'_>,
    problems: &mut Vec<ori_arc::ArcProblem>,
    exempt: &FxHashSet<u32>,
) -> Vec<crate::realization::ArcFunctionGroup> {
    let mut groups = Vec::new();
    for imported in input.imported.imported_mono_fns {
        let mono = &imported.function;
        let source_canon = input
            .imported
            .re_interned_canons
            .get(imported.module_index)
            .unwrap_or(input.canon);
        let mut context = crate::arc_lowering::ArcLoweringContext {
            canon: source_canon,
            interner: input.interner,
            pool: input.pool,
            problems,
        };
        let (function, lambdas) = match imported.body {
            super::imported_mono::ImportedMonoBody::Function(source_name) => {
                crate::arc_lowering::lower_to_arc(
                    mono.mangled_name,
                    &mono.sig,
                    source_name,
                    &mut context,
                    Some(&mono.body_type_map),
                )
            }
            super::imported_mono::ImportedMonoBody::ImplMethod(source_body) => {
                crate::arc_lowering::lower_impl_method_to_arc_by_source(
                    mono.mangled_name,
                    &mono.sig,
                    source_body,
                    &mut context,
                    Some(&mono.body_type_map),
                )
            }
        };
        super::pc2_hooks::run_pc2_hook_aot(
            input.pool,
            &function,
            &lambdas,
            input.interner,
            exempt,
            "aot_imported_mono",
            "aot_imported_mono_lambda",
        );
        groups.push(crate::realization::ArcFunctionGroup::new(function, lambdas));
    }
    groups
}

/// Lower every non-generic, specialized, and imported-specialized body.
///
/// Lowers each function (local, mono, imported mono) to ARC IR. Returns one
/// cache of pre-lowered ARC functions for zero-copy consumption by
/// shared executable realization and subsequent backend projection.
/// ARC lowering emits diagnostics. The returned lowered state has
/// not crossed the shared specialization and target-closure seam.
pub(super) fn lower_arc_batch(
    input: ArcBatchLoweringInput<'_>,
) -> Result<ArcBatchLoweringResult, ArcBatchLoweringFailure> {
    let mut arc_problems = Vec::new();
    let exempt = FxHashSet::default();
    let mut groups = lower_source_groups(&input, &mut arc_problems, &exempt)?;
    let imported_mono_fns = input.imported.imported_mono_fns;
    // Lower monomorphized generic functions.
    // Why: without mono entries in annotated_sigs, borrow lookup falls back to all-Owned.
    let mono_functions = ori_repr::monomorphize::collect_mono_functions(
        input.mono_instances,
        input.function_sigs,
        input.impl_sigs,
        input.accepted_derives,
        input.import_sigs,
        input.interner,
        input.pool,
    );
    let mono_groups = crate::realization::lower_mono_functions_for_analysis(
        &mono_functions,
        input.accepted_derives,
        input.derived_call_plans,
        input.canon,
        input.interner,
        input.pool,
        &mut arc_problems,
    );
    let mono_inventory = crate::realization::MonoFunctionInventory::try_new(
        mono_functions,
        imported_mono_fns
            .iter()
            .map(|imported| imported.function.clone()),
        input.interner,
    )
    .map_err(ArcBatchLoweringFailure::MonoInventory)?;
    for group in mono_groups {
        let (arc_fn, lambdas) = group.into_parts();
        super::pc2_hooks::run_pc2_hook_aot(
            input.pool,
            &arc_fn,
            &lambdas,
            input.interner,
            &exempt,
            "aot_mono",
            "aot_mono_lambda",
        );
        groups.push(crate::realization::ArcFunctionGroup::new(arc_fn, lambdas));
    }

    groups.extend(lower_imported_mono_groups(
        &input,
        &mut arc_problems,
        &exempt,
    ));
    if !arc_problems.is_empty() {
        use crate::problem::codegen::{emit_codegen_diagnostics, CodegenDiagnostics};
        let mut acc = CodegenDiagnostics::new();
        acc.add_arc_problems(&arc_problems);
        if emit_codegen_diagnostics(acc) {
            return Err(ArcBatchLoweringFailure::ArcLowering);
        }
    }

    Ok(ArcBatchLoweringResult {
        groups,
        mono_functions: mono_inventory.into_all(),
    })
}
