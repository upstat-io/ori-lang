use super::*;

/// Build a one-block unit-returning ARC function.
pub(super) fn minimal_unit_function(name: Name, body: Vec<ArcInstr>) -> ArcFunction {
    ArcFunction {
        name,
        params: Vec::new(),
        return_type: Idx::UNIT,
        blocks: vec![ArcBlock {
            id: ArcBlockId::new(0),
            params: Vec::new(),
            body,
            terminator: ArcTerminator::Return {
                value: ArcVarId::new(0),
            },
        }],
        entry: ArcBlockId::new(0),
        var_types: vec![Idx::UNIT],
        ..ArcFunction::default()
    }
}

/// Build a closed program that invokes catch recovery.
pub(super) fn catch_recover_executable() -> ExecutableProgram {
    let symbols = SharedInterner::new();
    let main = symbols.intern("main");
    let recover = symbols.intern("ori_catch_recover");
    let function = ArcFunction {
        name: main,
        params: Vec::new(),
        return_type: Idx::STR,
        blocks: vec![ArcBlock {
            id: ArcBlockId::new(0),
            params: Vec::new(),
            body: vec![ArcInstr::Apply {
                dst: ArcVarId::new(0),
                ty: Idx::STR,
                func: recover,
                args: Vec::new(),
                arg_ownership: Vec::new(),
                mono_instance_id: None,
            }],
            terminator: ArcTerminator::Return {
                value: ArcVarId::new(0),
            },
        }],
        entry: ArcBlockId::new(0),
        var_types: vec![Idx::STR],
        ..ArcFunction::default()
    };
    close_test_executable(
        symbols,
        Pool::new(),
        vec![function],
        vec![FunctionFamilyTopology::new(main, Vec::new())],
        main,
    )
}

/// Build a closed program containing the unsupported inject-trace operation.
pub(super) fn unsupported_compiler_operation_executable() -> ExecutableProgram {
    let symbols = SharedInterner::new();
    let main = symbols.intern("main");
    let inject_trace = symbols.intern("__ori_inject_trace");
    let function = ArcFunction {
        name: main,
        params: Vec::new(),
        return_type: Idx::UNIT,
        blocks: vec![ArcBlock {
            id: ArcBlockId::new(0),
            params: Vec::new(),
            body: vec![
                ArcInstr::Let {
                    dst: ArcVarId::new(0),
                    ty: Idx::UNIT,
                    value: ArcValue::Literal(LitValue::Unit),
                },
                ArcInstr::Apply {
                    dst: ArcVarId::new(1),
                    ty: Idx::UNIT,
                    func: inject_trace,
                    args: vec![ArcVarId::new(0)],
                    arg_ownership: vec![ArgOwnership::Borrowed],
                    mono_instance_id: None,
                },
            ],
            terminator: ArcTerminator::Return {
                value: ArcVarId::new(1),
            },
        }],
        entry: ArcBlockId::new(0),
        var_types: vec![Idx::UNIT, Idx::UNIT],
        ..ArcFunction::default()
    };
    close_test_executable(
        symbols,
        Pool::new(),
        vec![function],
        vec![FunctionFamilyTopology::new(main, Vec::new())],
        main,
    )
}

/// Realize and validate a closed list-concatenation program.
pub(super) fn list_concat_executable() -> ExecutableProgram {
    let symbols = SharedInterner::new();
    let main = symbols.intern("main");
    let mut pool = Pool::new();
    let list_type = pool.list(Idx::INT);

    let mut functions = vec![list_concat_function(main, list_type)];
    let classifier = ArcClassifier::new(&pool);
    let builtins = BuiltinOwnershipSets::new(&symbols);
    let type_registry = TypeRegistry::new();
    let external_contracts: std::collections::HashMap<ori_ir::Name, ori_arc::MemoryContract> =
        std::collections::HashMap::default();
    let callable_boundaries = ori_arc::CallableBoundaryFacts::default();
    let realization = realize_closed_program(
        &mut functions,
        &ori_arc::ArcPipelineContext {
            classifier: &classifier,
            interner: &symbols,
            pool: &pool,
            builtins: &builtins,
            type_registry: &type_registry,
            callable_boundaries: &callable_boundaries,
            verify_arc: true,
            external_contracts: &external_contracts,
        },
    )
    .unwrap_or_else(|errors| panic!("list concat realization should succeed: {errors:?}"));

    ExecutableProgram::validate(ExecutableProgramParts {
        version: EXECUTABLE_PROGRAM_VERSION,
        symbols,
        pool,
        functions,
        function_families: vec![FunctionFamilyTopology::new(main, Vec::new())],
        contracts: realization.contracts,
        function_effects: realization.function_effects,
        fresh_return_facts: realization.fresh_return_facts,
        param_disjointness: realization.param_disjointness,
        callable_facts: realization.callable_facts,
        closure_adapters: realization.closure_adapters,
        retain_plans: realization.retain_plans,
        roots: vec![main],
        cli_entry: Some(main),
        externals: ori_repr::executable::ValidatedExternalCallables::empty(),
        method_targets: FxHashMap::default(),
        user_drop_bindings: Vec::new(),
        repr_plan: ReprPlan::new(NarrowingPolicy::Disabled),
        type_registry,
    })
    .unwrap_or_else(|error| panic!("list concat executable should validate: {error}"))
}

/// Build the ARC function used by the list-concatenation fixture.
pub(super) fn list_concat_function(main: Name, list_type: Idx) -> ArcFunction {
    ArcFunction {
        name: main,
        params: Vec::new(),
        return_type: list_type,
        blocks: vec![ArcBlock {
            id: ArcBlockId::new(0),
            params: Vec::new(),
            body: vec![
                ArcInstr::Construct {
                    dst: ArcVarId::new(0),
                    ty: list_type,
                    ctor: CtorKind::ListLiteral,
                    args: Vec::new(),
                },
                ArcInstr::Construct {
                    dst: ArcVarId::new(1),
                    ty: list_type,
                    ctor: CtorKind::ListLiteral,
                    args: Vec::new(),
                },
                ArcInstr::Let {
                    dst: ArcVarId::new(2),
                    ty: list_type,
                    value: ArcValue::PrimOp {
                        op: PrimOp::Binary(BinaryOp::Add),
                        args: vec![ArcVarId::new(0), ArcVarId::new(1)],
                    },
                },
            ],
            terminator: ArcTerminator::Return {
                value: ArcVarId::new(2),
            },
        }],
        entry: ArcBlockId::new(0),
        var_types: vec![list_type; 3],
        var_reprs: Vec::new(),
        var_rc_strategies: Vec::new(),
        var_metadata_state: ori_arc::VariableMetadataState::Unrealized,
        spans: Vec::new(),
        is_fbip: false,
        num_captures: 0,
        cow_annotations: CowAnnotations::default(),
        primitive_facts: PrimitiveFacts::default(),
        drop_hints: DropHints::default(),
        tail_calls: Vec::new(),
        burden_emitted: Vec::new(),
        reassign_deaths: Vec::new(),
        catch_scoped_checked_ops: Vec::new(),
        method_call_facts: Vec::new(),
        operator_call_facts: Vec::new(),
        direct_call_facts: Vec::new(),
        yield_allocations: Vec::new(),
        class_ledger_emission: false,
    }
}
