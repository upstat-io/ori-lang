//! LLVM return-attribute projection from frozen backend-neutral RL-29 facts.

use ori_arc::FreshSelfAllocationFacts;
use ori_ir::Name;
use tracing::debug;

use super::FunctionCompiler;
use crate::codegen::abi::{FunctionAbi, ReturnPassing};
use crate::codegen::value_id::FunctionId;

impl<'scx: 'ctx, 'ctx> FunctionCompiler<'_, 'scx, 'ctx, '_> {
    /// Attach LLVM return `noalias` only from a matching closed artifact.
    pub(super) fn apply_return_attributes(
        &mut self,
        name: Name,
        function: FunctionId,
        abi: &FunctionAbi,
    ) {
        let Some(executable_function) = self.bound_executable_function_id(name, abi) else {
            return;
        };
        let program = self
            .executable_program
            .expect("bound executable identity requires an executable program");
        let return_is_pointer = self
            .type_resolver
            .resolve(abi.return_abi.ty)
            .is_pointer_type();
        if project_return_noalias(
            program.fresh_return_facts(executable_function),
            abi,
            return_is_pointer,
        ) {
            self.builder.add_noalias_return_attribute(function);
            debug!(
                name = %self.interner.lookup(name),
                "projected frozen fresh-self-allocation fact as return noalias"
            );
        }
    }
}

fn project_return_noalias(
    facts: FreshSelfAllocationFacts,
    abi: &FunctionAbi,
    return_is_pointer: bool,
) -> bool {
    facts.is_proven()
        && return_is_pointer
        && matches!(abi.return_abi.passing, ReturnPassing::Direct)
}

#[cfg(test)]
mod tests {
    use std::mem::ManuallyDrop;

    use inkwell::context::Context;
    use ori_arc::{
        prove_param_disjointness, AnnotatedSig, ArcClassifier, ArcFunction, MemoryContract,
        RetainPlanTable, VariableMetadataState,
    };
    use ori_ir::{Name, SharedInterner};
    use ori_repr::executable::{
        ExecutableProgram, ExecutableProgramParts, EXECUTABLE_PROGRAM_VERSION,
    };
    use ori_repr::{NarrowingPolicy, ReprPlan};
    use ori_types::{Idx, Pool, TypeRegistry};
    use rustc_hash::FxHashMap;

    use super::project_return_noalias;
    use crate::codegen::abi::{CallConv, FunctionAbi, ReturnAbi, ReturnPassing};
    use crate::codegen::function_compiler::FunctionCompiler;
    use crate::codegen::ir_builder::IrBuilder;
    use crate::codegen::type_info::{TypeInfoStore, TypeLayoutResolver};
    use crate::context::SimpleCx;

    fn abi(ty: Idx, passing: ReturnPassing) -> FunctionAbi {
        FunctionAbi {
            params: Vec::new(),
            return_abi: ReturnAbi { ty, passing },
            call_conv: CallConv::Fast,
        }
    }

    fn llvm_declaration<'a>(ir: &'a str, name: &str) -> &'a str {
        ir.lines()
            .find(|line| line.contains(&format!("@{name}(")))
            .unwrap_or_else(|| panic!("missing {name} declaration:\n{ir}"))
    }

    #[test]
    fn neutral_fact_requires_fresh_self_allocation_and_pointer_abi() {
        let mut fresh = MemoryContract::conservative(0);
        fresh.return_info.returns_fresh_self_alloc = true;
        let fresh_facts = fresh.fresh_self_allocation_facts();
        let direct = abi(Idx::INT, ReturnPassing::Direct);
        assert!(project_return_noalias(fresh_facts, &direct, true));
        assert!(!project_return_noalias(fresh_facts, &direct, false));
        assert!(!project_return_noalias(
            fresh_facts,
            &abi(Idx::INT, ReturnPassing::Sret { alignment: 8 }),
            true,
        ));

        let mut forwarded_or_consumed = fresh;
        forwarded_or_consumed.return_info.preserves_freshness = true;
        forwarded_or_consumed.return_info.returns_fresh_self_alloc = false;
        assert!(!project_return_noalias(
            forwarded_or_consumed.fresh_self_allocation_facts(),
            &direct,
            true,
        ));
    }

    struct ReturnProjectionFixture {
        program: ExecutableProgram,
        symbols: SharedInterner,
        channel: Idx,
        fresh_name: Name,
        passthrough_name: Name,
        consumed_name: Name,
        legacy_name: Name,
    }

    fn return_contracts(
        fresh_name: Name,
        passthrough_name: Name,
        consumed_name: Name,
    ) -> FxHashMap<Name, MemoryContract> {
        let mut fresh = MemoryContract::conservative(0);
        fresh.return_info.returns_fresh_self_alloc = true;
        let mut passthrough = MemoryContract::conservative(0);
        passthrough.return_info.preserves_freshness = true;
        let mut consumed = MemoryContract::conservative(0);
        consumed.return_info.preserves_freshness = true;
        [
            (fresh_name, fresh),
            (passthrough_name, passthrough),
            (consumed_name, consumed),
        ]
        .into_iter()
        .collect()
    }

    fn return_projection_fixture() -> ReturnProjectionFixture {
        let symbols = SharedInterner::new();
        let fresh_name = symbols.intern("fresh_return");
        let passthrough_name = symbols.intern("passthrough_return");
        let consumed_name = symbols.intern("consumed_storage_return");
        let legacy_name = symbols.intern("legacy_return");
        let mut pool = Pool::new();
        let channel = pool.channel(Idx::INT);
        let functions: Vec<_> = [fresh_name, passthrough_name, consumed_name]
            .into_iter()
            .map(|name| ArcFunction {
                name,
                return_type: channel,
                var_metadata_state: VariableMetadataState::Realized,
                ..ArcFunction::default()
            })
            .collect();

        let contracts = return_contracts(fresh_name, passthrough_name, consumed_name);
        let function_effects = functions
            .iter()
            .map(|function| {
                (
                    function.name,
                    contracts[&function.name].function_effect_facts(function),
                )
            })
            .collect();
        let fresh_return_facts = functions
            .iter()
            .map(|function| {
                (
                    function.name,
                    contracts[&function.name].fresh_self_allocation_facts(),
                )
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
            function_families: [fresh_name, passthrough_name, consumed_name]
                .into_iter()
                .map(|name| ori_repr::executable::FunctionFamilyTopology::new(name, Vec::new()))
                .collect(),
            contracts,
            function_effects,
            fresh_return_facts,
            param_disjointness,
            callable_facts,
            closure_adapters: FxHashMap::default(),
            retain_plans: RetainPlanTable::default(),
            roots: vec![fresh_name],
            cli_entry: Some(fresh_name),
            externals: Vec::new(),
            user_drop_bindings: Vec::new(),
            repr_plan: ReprPlan::new(NarrowingPolicy::Disabled),
            type_registry: TypeRegistry::new(),
        })
        .unwrap_or_else(|error| panic!("return-fact artifact must validate: {error}"));

        ReturnProjectionFixture {
            program,
            symbols,
            channel,
            fresh_name,
            passthrough_name,
            consumed_name,
            legacy_name,
        }
    }

    fn emit_return_projection_ir(fixture: &ReturnProjectionFixture) -> String {
        let context = Context::create();
        let store = TypeInfoStore::new(fixture.program.pool());
        let simple = ManuallyDrop::new(SimpleCx::new(&context, "return_projection"));
        let resolver = TypeLayoutResolver::new(&store, &simple, Some(&fixture.symbols), None);
        let mut builder = IrBuilder::new(&simple);
        let classifier = ArcClassifier::new(fixture.program.pool());
        let annotated_sigs: FxHashMap<Name, AnnotatedSig> = FxHashMap::default();
        let mut compiler = FunctionCompiler::new(
            &mut builder,
            &store,
            &resolver,
            &fixture.symbols,
            fixture.program.pool(),
            "",
            &annotated_sigs,
            &classifier,
            None,
            false,
        );
        let abi = abi(fixture.channel, ReturnPassing::Direct);
        let legacy_function = compiler.declare_function_llvm("legacy_return", &abi);
        compiler.apply_return_attributes(fixture.legacy_name, legacy_function, &abi);
        compiler.bind_executable_program(&fixture.program);
        for (name, spelling) in [
            (fixture.fresh_name, "fresh_return"),
            (fixture.passthrough_name, "passthrough_return"),
            (fixture.consumed_name, "consumed_storage_return"),
        ] {
            let function = compiler.declare_function_llvm(spelling, &abi);
            compiler.apply_return_attributes(name, function, &abi);
        }
        drop(compiler);
        drop(builder);
        drop(resolver);

        simple.llmod.print_to_string().to_string()
    }

    #[test]
    fn bound_artifact_projects_only_proven_fresh_pointer_returns() {
        let fixture = return_projection_fixture();
        let ir = emit_return_projection_ir(&fixture);

        assert!(llvm_declaration(&ir, "fresh_return").contains("noalias ptr"));
        assert!(!llvm_declaration(&ir, "passthrough_return").contains("noalias"));
        assert!(!llvm_declaration(&ir, "consumed_storage_return").contains("noalias"));
        assert!(!llvm_declaration(&ir, "legacy_return").contains("noalias"));
    }
}
