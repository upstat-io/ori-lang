//! Identity checks and generic family projection for executable-program facts.

use ori_ir::Name;
use rustc_hash::FxHashSet;

use super::FunctionCompiler;
use crate::codegen::abi::{compute_function_abi_from_shape, CallConvSite, FunctionAbi};

impl<'scx: 'ctx, 'ctx> FunctionCompiler<'_, 'scx, 'ctx, '_> {
    /// Declare every closed artifact parent not owned by an earlier physical role.
    ///
    /// Source functions, monomorphized functions, imports, and impl methods have
    /// role-specific symbol or dispatch registration and are declared before
    /// this pass. `deferred_parents` names roles which are intentionally emitted
    /// later through a custom projection, currently impl methods and JIT test
    /// wrappers. Every remaining parent is a compiler-generated executable body
    /// and uses its exact artifact shape plus frozen AIMS ownership contract to
    /// derive one physical ABI.
    ///
    /// The returned stable artifact-identity list is the only inventory consumed by the
    /// matching preparation pass. This prevents a backend-local scan from
    /// rediscovering a different body set after declaration.
    pub fn declare_artifact_remainder(
        &mut self,
        deferred_parents: &[Name],
    ) -> Vec<ori_repr::executable::FunctionId> {
        let Some(program) = self.executable_program else {
            self.builder.record_codegen_error_with_msg(
                "LLVM artifact-family declaration requires a closed executable program",
            );
            return Vec::new();
        };

        let declared: FxHashSet<Name> = self.codegen_ctx.functions.keys().copied().collect();
        let deferred: FxHashSet<Name> = deferred_parents.iter().copied().collect();
        let claimed_functions: Vec<_> = program
            .functions()
            .iter()
            .filter_map(|function| {
                let function_id = program
                    .function_id(function.name)
                    .unwrap_or_else(|| unreachable!("validated function has no stable identity"));
                (program.function_family_lambdas(function_id).is_some()
                    && declared.contains(&function.name))
                .then_some(function_id)
            })
            .collect();
        let functions = unclaimed_artifact_parents(program, &declared, &deferred);

        // A role-specific declaration may choose a different symbol, but its
        // physical ABI must still be the projection of the same closed facts.
        for function_id in claimed_functions {
            let name = self
                .executable_program
                .unwrap_or_else(|| unreachable!("closed executable disappeared"))
                .function(function_id)
                .name;
            let declared_abi = self.codegen_ctx.functions.get(&name).map_or_else(
                || unreachable!("claimed declaration disappeared"),
                |(_, abi)| abi.clone(),
            );
            let Some(expected_abi) = self.compute_artifact_parent_abi(function_id) else {
                continue;
            };
            if !same_physical_abi(&declared_abi, &expected_abi) {
                self.builder.record_codegen_error_with_msg(format!(
                    "LLVM declaration for {} disagrees with the physical ABI projected from its closed executable facts",
                    self.interner.lookup(name)
                ));
            }
        }

        let mut declared_remainder = Vec::with_capacity(functions.len());
        for function_id in functions {
            let name = self
                .executable_program
                .unwrap_or_else(|| unreachable!("closed executable disappeared"))
                .function(function_id)
                .name;
            let Some(abi) = self.compute_artifact_parent_abi(function_id) else {
                continue;
            };
            let symbol = self
                .mangler
                .mangle_function(self.module_path, self.interner.lookup(name));
            let function = self.declare_function_llvm(&symbol, &abi);
            self.codegen_ctx
                .functions
                .insert(name, (function, abi.clone()));
            self.declare_length_projection_clone(name, &abi);
            declared_remainder.push(function_id);
        }
        declared_remainder
    }

    /// Project one parent ABI solely from closed executable facts.
    fn compute_artifact_parent_abi(
        &mut self,
        function_id: ori_repr::executable::FunctionId,
    ) -> Option<FunctionAbi> {
        let program = self.executable_program?;
        let function = program.function(function_id);
        let annotated = program
            .function_contract(function_id)
            .to_annotated_sig(&function.params, function.return_type);

        let params = function
            .params
            .iter()
            .zip(&annotated.params)
            .map(|(parameter, fact)| {
                (
                    self.interner.intern(&format!("v{}", parameter.var.raw())),
                    parameter.ty,
                    fact.ownership,
                )
            });
        let site = if program.cli_entry() == Some(function_id) {
            CallConvSite::Main
        } else {
            CallConvSite::OriFunction
        };
        Some(compute_function_abi_from_shape(
            params,
            function.return_type,
            site,
            self.type_info,
            Some(self.arc_classifier),
            self.repr_plan(),
        ))
    }

    /// Resolve an LLVM function to a validated stable executable identity.
    pub(super) fn bound_executable_function_id(
        &self,
        name: Name,
        abi: &FunctionAbi,
    ) -> Option<ori_repr::executable::FunctionId> {
        match_executable_function(self.executable_program, self.interner, name, abi)
    }

    /// Clone one exact parent/lambda family from the bound executable artifact.
    ///
    /// Source canonical IR and pre-AIMS caches are deliberately not fallback
    /// authorities here. A missing family is a closed-program construction
    /// error and prevents physical emission.
    pub(super) fn clone_bound_family(
        &mut self,
        name: Name,
        abi: &FunctionAbi,
    ) -> Option<(ori_arc::ArcFunction, Vec<ori_arc::ArcFunction>)> {
        let Some(program) = self.executable_program else {
            self.builder.record_codegen_error_with_msg(format!(
                "physical LLVM projection requires a closed executable family for {}",
                self.interner.lookup(name)
            ));
            return None;
        };
        let Some(function) = self.bound_executable_function_id(name, abi) else {
            self.builder.record_codegen_error_with_msg(format!(
                "closed executable body for {} is missing or disagrees with its physical ABI",
                self.interner.lookup(name)
            ));
            return None;
        };
        let Some(lambda_ids) = program.function_family_lambdas(function) else {
            self.builder.record_codegen_error_with_msg(format!(
                "closed executable body for {} is a nested lambda, not a family parent",
                self.interner.lookup(name)
            ));
            return None;
        };
        let parent = program.function(function).clone();
        let lambdas = lambda_ids
            .iter()
            .map(|&lambda| program.function(lambda).clone())
            .collect();
        Some((parent, lambdas))
    }
}

fn unclaimed_artifact_parents(
    program: &ori_repr::executable::ExecutableProgram,
    declared: &FxHashSet<Name>,
    deferred: &FxHashSet<Name>,
) -> Vec<ori_repr::executable::FunctionId> {
    program
        .functions()
        .iter()
        .filter_map(|function| {
            let function_id = program
                .function_id(function.name)
                .unwrap_or_else(|| unreachable!("validated function has no stable identity"));
            program
                .function_family_lambdas(function_id)
                .is_some()
                .then_some(function_id)
        })
        .filter(|&function| {
            let name = program.function(function).name;
            !declared.contains(&name) && !deferred.contains(&name)
        })
        .collect()
}

fn same_physical_abi(left: &FunctionAbi, right: &FunctionAbi) -> bool {
    left.call_conv == right.call_conv
        && left.return_abi == right.return_abi
        && left.params.len() == right.params.len()
        && left.params.iter().zip(&right.params).all(|(left, right)| {
            left.ty == right.ty && left.passing == right.passing && left.readonly == right.readonly
        })
}

fn match_executable_function(
    program: Option<&ori_repr::executable::ExecutableProgram>,
    interner: &ori_ir::StringInterner,
    name: Name,
    abi: &FunctionAbi,
) -> Option<ori_repr::executable::FunctionId> {
    let program = program?;
    let function = program.function_id(name)?;
    let artifact_function = program.function(function);
    if program.symbols().lookup(artifact_function.name) != interner.lookup(name)
        || artifact_function.return_type != abi.return_abi.ty
        || artifact_function.params.len() != abi.params.len()
        || artifact_function
            .params
            .iter()
            .zip(&abi.params)
            .any(|(artifact, llvm)| artifact.ty != llvm.ty)
    {
        return None;
    }
    Some(function)
}

#[cfg(test)]
mod tests {
    use ori_arc::{
        prove_param_disjointness, ArcFunction, MemoryContract, RetainPlanTable,
        VariableMetadataState,
    };
    use ori_ir::SharedInterner;
    use ori_repr::executable::{
        ExecutableProgram, ExecutableProgramParts, FunctionFamilyTopology,
        EXECUTABLE_PROGRAM_VERSION,
    };
    use ori_repr::{NarrowingPolicy, ReprPlan};
    use ori_types::{Idx, Pool, TypeRegistry};
    use rustc_hash::{FxHashMap, FxHashSet};

    use super::{match_executable_function, same_physical_abi, unclaimed_artifact_parents};
    use crate::codegen::abi::{
        CallConv, FunctionAbi, ParamAbi, ParamPassing, ReturnAbi, ReturnPassing,
    };

    fn fixture() -> (ExecutableProgram, SharedInterner, ori_ir::Name) {
        let symbols = SharedInterner::new();
        let main = symbols.intern("main");
        let function = ArcFunction {
            name: main,
            return_type: Idx::INT,
            var_metadata_state: VariableMetadataState::Realized,
            ..ArcFunction::default()
        };
        let contract = MemoryContract::conservative(0);
        let function_effects = [(main, contract.function_effect_facts(&function))]
            .into_iter()
            .collect();
        let fresh_return_facts = [(main, contract.fresh_self_allocation_facts())]
            .into_iter()
            .collect();
        let pool = Pool::new();
        let param_disjointness = [(main, prove_param_disjointness(&[], &pool))]
            .into_iter()
            .collect();
        let callable_facts =
            ori_arc::freeze_function_callable_facts(std::slice::from_ref(&function), &pool);
        let contracts = [(main, contract)].into_iter().collect();
        let program = ExecutableProgram::validate(ExecutableProgramParts {
            version: EXECUTABLE_PROGRAM_VERSION,
            symbols: symbols.clone(),
            pool,
            functions: vec![function],
            function_families: vec![ori_repr::executable::FunctionFamilyTopology::new(
                main,
                Vec::new(),
            )],
            contracts,
            function_effects,
            fresh_return_facts,
            param_disjointness,
            callable_facts,
            closure_adapters: FxHashMap::default(),
            retain_plans: RetainPlanTable::default(),
            roots: vec![main],
            cli_entry: Some(main),
            externals: Vec::new(),
            method_targets: FxHashMap::default(),
            user_drop_bindings: Vec::new(),
            repr_plan: ReprPlan::new(NarrowingPolicy::Disabled),
            type_registry: TypeRegistry::new(),
        })
        .unwrap_or_else(|error| panic!("artifact fixture must validate: {error}"));
        (program, symbols, main)
    }

    fn abi(return_type: Idx) -> FunctionAbi {
        FunctionAbi {
            params: Vec::new(),
            return_abi: ReturnAbi {
                ty: return_type,
                passing: ReturnPassing::Direct,
            },
            call_conv: CallConv::Fast,
        }
    }

    fn artifact_role_fixture() -> (
        ExecutableProgram,
        ori_ir::Name,
        ori_ir::Name,
        ori_ir::Name,
        ori_ir::Name,
    ) {
        let symbols = SharedInterner::new();
        let source = symbols.intern("source");
        let generated = symbols.intern("generated");
        let generated_lambda = symbols.intern("generated.lambda");
        let deferred_impl = symbols.intern("deferred.impl");
        let deferred_test = symbols.intern("deferred.test");
        let functions: Vec<_> = [
            source,
            generated,
            generated_lambda,
            deferred_impl,
            deferred_test,
        ]
        .into_iter()
        .map(|name| ArcFunction {
            name,
            return_type: Idx::UNIT,
            var_metadata_state: VariableMetadataState::Realized,
            ..ArcFunction::default()
        })
        .collect();
        let pool = Pool::new();
        let contracts = functions
            .iter()
            .map(|function| {
                (
                    function.name,
                    MemoryContract::conservative(function.params.len()),
                )
            })
            .collect();
        let function_effects = functions
            .iter()
            .map(|function| {
                let contract = MemoryContract::conservative(function.params.len());
                (function.name, contract.function_effect_facts(function))
            })
            .collect();
        let fresh_return_facts = functions
            .iter()
            .map(|function| {
                let contract = MemoryContract::conservative(function.params.len());
                (function.name, contract.fresh_self_allocation_facts())
            })
            .collect();
        let param_disjointness = functions
            .iter()
            .map(|function| (function.name, prove_param_disjointness(&[], &pool)))
            .collect();
        let callable_facts = ori_arc::freeze_function_callable_facts(&functions, &pool);
        let program = ExecutableProgram::validate(ExecutableProgramParts {
            version: EXECUTABLE_PROGRAM_VERSION,
            symbols: symbols.clone(),
            pool,
            functions,
            function_families: vec![
                FunctionFamilyTopology::new(source, Vec::new()),
                FunctionFamilyTopology::new(generated, vec![generated_lambda]),
                FunctionFamilyTopology::new(deferred_impl, Vec::new()),
                FunctionFamilyTopology::new(deferred_test, Vec::new()),
            ],
            contracts,
            function_effects,
            fresh_return_facts,
            param_disjointness,
            callable_facts,
            closure_adapters: FxHashMap::default(),
            retain_plans: RetainPlanTable::default(),
            roots: vec![source],
            cli_entry: Some(source),
            externals: Vec::new(),
            method_targets: FxHashMap::default(),
            user_drop_bindings: Vec::new(),
            repr_plan: ReprPlan::new(NarrowingPolicy::Disabled),
            type_registry: TypeRegistry::new(),
        })
        .unwrap_or_else(|error| panic!("artifact-role fixture must validate: {error}"));
        (program, source, generated, deferred_impl, deferred_test)
    }

    fn parameterized_abi(name: ori_ir::Name) -> FunctionAbi {
        FunctionAbi {
            params: vec![ParamAbi {
                name,
                ty: Idx::INT,
                passing: ParamPassing::Direct,
                readonly: false,
            }],
            return_abi: ReturnAbi {
                ty: Idx::INT,
                passing: ReturnPassing::Direct,
            },
            call_conv: CallConv::Fast,
        }
    }

    #[test]
    fn exact_artifact_identity_binds_stable_function_id() {
        let (program, symbols, main) = fixture();
        let function = match_executable_function(Some(&program), &symbols, main, &abi(Idx::INT))
            .unwrap_or_else(|| panic!("matching artifact must bind"));

        assert_eq!(function, program.cli_entry().expect("fixture CLI entry"));
    }

    #[test]
    fn missing_or_mismatched_artifact_fails_closed() {
        let (program, symbols, main) = fixture();
        let missing = symbols.intern("missing");
        assert!(match_executable_function(None, &symbols, main, &abi(Idx::INT)).is_none());
        assert!(
            match_executable_function(Some(&program), &symbols, missing, &abi(Idx::INT),).is_none()
        );
        assert!(
            match_executable_function(Some(&program), &symbols, main, &abi(Idx::BOOL),).is_none()
        );
    }

    #[test]
    fn unclaimed_artifact_parents_mixed_roles_returns_only_generated_parent() {
        let (program, source, generated, deferred_impl, deferred_test) = artifact_role_fixture();
        let declared = FxHashSet::from_iter([source]);
        let deferred = FxHashSet::from_iter([deferred_impl, deferred_test]);
        let generated_id = program
            .function_id(generated)
            .unwrap_or_else(|| panic!("generated parent must have a stable identity"));

        assert_eq!(
            unclaimed_artifact_parents(&program, &declared, &deferred),
            vec![generated_id]
        );
    }

    #[test]
    fn same_physical_abi_different_parameter_names_matches() {
        let symbols = SharedInterner::new();
        let left = parameterized_abi(symbols.intern("left_display_name"));
        let right = parameterized_abi(symbols.intern("right_display_name"));

        assert!(same_physical_abi(&left, &right));
    }

    #[test]
    fn same_physical_abi_parameter_passing_mismatch_rejects() {
        let symbols = SharedInterner::new();
        let left = parameterized_abi(symbols.intern("parameter"));
        let mut right = left.clone();
        right.params[0].passing = ParamPassing::Reference;

        assert!(!same_physical_abi(&left, &right));
    }

    #[test]
    fn same_physical_abi_parameter_readonly_mismatch_rejects() {
        let symbols = SharedInterner::new();
        let left = parameterized_abi(symbols.intern("parameter"));
        let mut right = left.clone();
        right.params[0].readonly = true;

        assert!(!same_physical_abi(&left, &right));
    }

    #[test]
    fn same_physical_abi_parameter_type_mismatch_rejects() {
        let symbols = SharedInterner::new();
        let left = parameterized_abi(symbols.intern("parameter"));
        let mut right = left.clone();
        right.params[0].ty = Idx::BOOL;

        assert!(!same_physical_abi(&left, &right));
    }

    #[test]
    fn same_physical_abi_return_type_mismatch_rejects() {
        let symbols = SharedInterner::new();
        let left = parameterized_abi(symbols.intern("parameter"));
        let mut right = left.clone();
        right.return_abi.ty = Idx::BOOL;

        assert!(!same_physical_abi(&left, &right));
    }

    #[test]
    fn same_physical_abi_return_passing_mismatch_rejects() {
        let symbols = SharedInterner::new();
        let left = parameterized_abi(symbols.intern("parameter"));
        let mut right = left.clone();
        right.return_abi.passing = ReturnPassing::Sret { alignment: 8 };

        assert!(!same_physical_abi(&left, &right));
    }
}
