//! LLVM memory-attribute projection from frozen backend-neutral AIMS facts.

use ori_arc::{FunctionEffectFacts, MemoryAccessClass};
use ori_ir::Name;
use tracing::debug;

use super::FunctionCompiler;
use crate::codegen::abi::{FunctionAbi, ReturnPassing};
use crate::codegen::value_id::FunctionId;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ProjectedMemoryAttribute {
    Read,
}

impl<'scx: 'ctx, 'ctx> FunctionCompiler<'_, 'scx, 'ctx, '_> {
    /// Attach an LLVM memory attribute only from a matching closed artifact.
    pub(super) fn apply_effect_attributes(
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
        let attribute =
            project_memory_attribute(program.function_effects(executable_function), abi);
        if attribute == Some(ProjectedMemoryAttribute::Read) {
            self.builder.add_memory_read_attribute(function);
            debug!(
                name = %self.interner.lookup(name),
                "projected frozen AIMS facts as memory(read)"
            );
        }
    }
}

fn project_memory_attribute(
    facts: FunctionEffectFacts,
    abi: &FunctionAbi,
) -> Option<ProjectedMemoryAttribute> {
    if matches!(abi.return_abi.passing, ReturnPassing::Sret { .. }) {
        return None;
    }
    (facts.memory_access() == MemoryAccessClass::ReadOnly).then_some(ProjectedMemoryAttribute::Read)
}

#[cfg(test)]
mod tests {
    use std::mem::ManuallyDrop;

    use inkwell::context::Context;
    use ori_arc::aims::contract::FipContract;
    use ori_arc::aims::lattice::{AccessClass, Cardinality};
    use ori_arc::{
        prove_param_disjointness, AnnotatedSig, ArcClassifier, ArcFunction, EffectSummary,
        MemoryContract, RetainPlanTable,
    };
    use ori_ir::{Name, SharedInterner};
    use ori_repr::executable::{
        ExecutableProgram, ExecutableProgramParts, EXECUTABLE_PROGRAM_VERSION,
    };
    use ori_repr::{NarrowingPolicy, ReprPlan};
    use ori_types::{Idx, Pool, TypeRegistry};
    use rustc_hash::FxHashMap;

    use super::{project_memory_attribute, ProjectedMemoryAttribute};
    use crate::codegen::abi::{
        CallConv, FunctionAbi, ParamAbi, ParamPassing, ReturnAbi, ReturnPassing,
    };
    use crate::codegen::function_compiler::FunctionCompiler;
    use crate::codegen::ir_builder::IrBuilder;
    use crate::codegen::type_info::{TypeInfoStore, TypeLayoutResolver};
    use crate::context::SimpleCx;

    fn direct_abi(params: usize, return_passing: ReturnPassing) -> FunctionAbi {
        FunctionAbi {
            params: (0..params)
                .map(|index| ParamAbi {
                    name: Name::from_raw(u32::try_from(index + 1).unwrap_or(u32::MAX)),
                    ty: Idx::STR,
                    passing: ParamPassing::Reference,
                    readonly: true,
                })
                .collect(),
            return_abi: ReturnAbi {
                ty: Idx::INT,
                passing: return_passing,
            },
            call_conv: CallConv::Fast,
        }
    }

    fn borrowed_read_contract(params: usize) -> MemoryContract {
        let mut contract = MemoryContract::all_borrowed(params, FipContract::Never);
        contract.effects = EffectSummary::OPTIMISTIC;
        for param in &mut contract.params {
            param.access = AccessClass::Borrowed;
            param.cardinality = Cardinality::Once;
            param.may_share = false;
        }
        contract
    }

    fn realized_effect_facts(contract: &MemoryContract) -> ori_arc::FunctionEffectFacts {
        contract.function_effect_facts(&ArcFunction::default())
    }

    fn llvm_attributes_for<'a>(ir: &'a str, declaration: &'a str) -> &'a str {
        declaration
            .split_whitespace()
            .find(|token| token.starts_with('#'))
            .and_then(|group| {
                ir.lines()
                    .find(|line| line.starts_with(&format!("attributes {group} =")))
            })
            .unwrap_or(declaration)
    }

    fn llvm_declaration<'a>(ir: &'a str, name: &str) -> &'a str {
        ir.lines()
            .find(|line| line.contains(&format!("@{name}(")))
            .unwrap_or_else(|| panic!("missing {name} declaration:\n{ir}"))
    }

    #[test]
    fn frozen_borrowed_no_write_facts_project_generic_memory_read() {
        let contract = borrowed_read_contract(1);
        let attribute = project_memory_attribute(
            realized_effect_facts(&contract),
            &direct_abi(1, ReturnPassing::Direct),
        );

        assert_eq!(attribute, Some(ProjectedMemoryAttribute::Read));
    }

    #[test]
    fn absent_params_still_project_read_not_memory_none() {
        let contract = borrowed_read_contract(0);
        let attribute = project_memory_attribute(
            realized_effect_facts(&contract),
            &direct_abi(0, ReturnPassing::Direct),
        );

        assert_eq!(attribute, Some(ProjectedMemoryAttribute::Read));
    }

    #[test]
    fn every_write_effect_omits_memory_none_and_memory_read() {
        let cases = [
            EffectSummary {
                may_allocate: true,
                ..EffectSummary::OPTIMISTIC
            },
            EffectSummary {
                may_deallocate: true,
                ..EffectSummary::OPTIMISTIC
            },
            EffectSummary {
                may_share: true,
                ..EffectSummary::OPTIMISTIC
            },
            EffectSummary {
                may_throw: true,
                ..EffectSummary::OPTIMISTIC
            },
        ];
        for effects in cases {
            let mut contract = borrowed_read_contract(1);
            contract.effects = effects;
            assert_eq!(
                project_memory_attribute(
                    realized_effect_facts(&contract),
                    &direct_abi(1, ReturnPassing::Direct),
                ),
                None
            );
        }
    }

    #[test]
    fn writable_params_and_sret_results_omit_memory_attributes() {
        let mut owned = borrowed_read_contract(1);
        owned.params[0].access = AccessClass::Owned;
        assert_eq!(
            project_memory_attribute(
                realized_effect_facts(&owned),
                &direct_abi(1, ReturnPassing::Direct),
            ),
            None
        );

        let mut sharing = borrowed_read_contract(1);
        sharing.params[0].may_share = true;
        assert_eq!(
            project_memory_attribute(
                realized_effect_facts(&sharing),
                &direct_abi(1, ReturnPassing::Direct),
            ),
            None
        );

        let read_only = borrowed_read_contract(1);
        assert_eq!(
            project_memory_attribute(
                realized_effect_facts(&read_only),
                &direct_abi(1, ReturnPassing::Sret { alignment: 8 }),
            ),
            None
        );
    }

    struct EffectProjectionFixture {
        program: ExecutableProgram,
        symbols: SharedInterner,
        read_name: Name,
        write_name: Name,
        throw_name: Name,
        legacy_name: Name,
    }

    fn effect_contracts(
        read_name: Name,
        write_name: Name,
        throw_name: Name,
    ) -> FxHashMap<Name, MemoryContract> {
        let mut write_contract = borrowed_read_contract(0);
        write_contract.effects.may_allocate = true;
        let mut throw_contract = borrowed_read_contract(0);
        throw_contract.effects.may_throw = true;
        [
            (read_name, borrowed_read_contract(0)),
            (write_name, write_contract),
            (throw_name, throw_contract),
        ]
        .into_iter()
        .collect()
    }

    fn effect_projection_fixture() -> EffectProjectionFixture {
        let symbols = SharedInterner::new();
        let read_name = symbols.intern("read_effects");
        let write_name = symbols.intern("write_effects");
        let throw_name = symbols.intern("throw_effects");
        let legacy_name = symbols.intern("legacy_effects");
        let functions: Vec<_> = [read_name, write_name, throw_name]
            .into_iter()
            .map(|name| ArcFunction {
                name,
                return_type: Idx::INT,
                var_metadata_state: ori_arc::VariableMetadataState::Realized,
                ..ArcFunction::default()
            })
            .collect();
        let contracts = effect_contracts(read_name, write_name, throw_name);
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
        let pool = Pool::new();
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
            function_families: [read_name, write_name, throw_name]
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
            roots: vec![read_name],
            cli_entry: Some(read_name),
            externals: ori_repr::executable::ValidatedExternalCallables::empty(),
            method_targets: FxHashMap::default(),
            user_drop_bindings: Vec::new(),
            repr_plan: ReprPlan::new(NarrowingPolicy::Disabled),
            type_registry: TypeRegistry::new(),
        })
        .unwrap_or_else(|error| panic!("effect artifact must validate: {error}"));

        EffectProjectionFixture {
            program,
            symbols,
            read_name,
            write_name,
            throw_name,
            legacy_name,
        }
    }

    fn emit_effect_projection_ir(fixture: &EffectProjectionFixture) -> String {
        let context = Context::create();
        let store = TypeInfoStore::new(fixture.program.pool());
        let simple = ManuallyDrop::new(SimpleCx::new(&context, "effect_projection"));
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
        let abi = direct_abi(0, ReturnPassing::Direct);
        let legacy_function = compiler.declare_function_llvm("legacy_effects", &abi);
        compiler.apply_effect_attributes(fixture.legacy_name, legacy_function, &abi);
        compiler.bind_executable_program(&fixture.program);
        let read_function = compiler.declare_function_llvm("read_effects", &abi);
        compiler.apply_effect_attributes(fixture.read_name, read_function, &abi);
        let write_function = compiler.declare_function_llvm("write_effects", &abi);
        compiler.apply_effect_attributes(fixture.write_name, write_function, &abi);
        let throw_function = compiler.declare_function_llvm("throw_effects", &abi);
        compiler.apply_effect_attributes(fixture.throw_name, throw_function, &abi);
        drop(compiler);
        drop(builder);
        drop(resolver);

        simple.llmod.print_to_string().to_string()
    }

    #[test]
    fn bound_artifact_emits_read_only_attribute_and_effectful_artifact_omits_both() {
        let fixture = effect_projection_fixture();
        let ir = emit_effect_projection_ir(&fixture);
        let read_line = llvm_declaration(&ir, "read_effects");
        let write_line = llvm_declaration(&ir, "write_effects");
        let legacy_line = llvm_declaration(&ir, "legacy_effects");
        let throw_line = llvm_declaration(&ir, "throw_effects");
        let read_attributes = llvm_attributes_for(&ir, read_line);
        let write_attributes = llvm_attributes_for(&ir, write_line);
        assert!(
            read_attributes.contains("memory(argmem: read, inaccessiblemem: read, errnomem: read)"),
            "{read_line}\n{read_attributes}"
        );
        assert!(!read_attributes.contains("none"), "{read_attributes}");
        assert!(!read_attributes.contains("write"), "{read_attributes}");
        assert!(!write_attributes.contains("memory("), "{write_attributes}");
        assert!(
            !llvm_attributes_for(&ir, throw_line).contains("memory("),
            "{throw_line}"
        );
        assert!(
            !llvm_attributes_for(&ir, legacy_line).contains("memory("),
            "{legacy_line}"
        );
    }
}
