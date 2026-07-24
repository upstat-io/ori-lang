use super::*;

/// Build a closed capture-bearing closure program and return its main/lambda identities.
pub(super) fn closure_executable() -> (ExecutableProgram, Name, Name) {
    let symbols = SharedInterner::new();
    let main = symbols.intern("main");
    let lambda = symbols.intern("__lambda_main_0");
    let mut pool = Pool::new();
    let closure_type = pool.function(&[Idx::INT], Idx::INT);

    let main_function = ArcFunction {
        name: main,
        params: Vec::new(),
        return_type: Idx::INT,
        blocks: vec![ArcBlock {
            id: ArcBlockId::new(0),
            params: Vec::new(),
            body: vec![
                ArcInstr::Let {
                    dst: ArcVarId::new(0),
                    ty: Idx::INT,
                    value: ArcValue::Literal(LitValue::Int(7)),
                },
                ArcInstr::PartialApply {
                    dst: ArcVarId::new(1),
                    ty: closure_type,
                    func: lambda,
                    args: vec![ArcVarId::new(0)],
                },
                ArcInstr::Let {
                    dst: ArcVarId::new(2),
                    ty: Idx::INT,
                    value: ArcValue::Literal(LitValue::Int(11)),
                },
                ArcInstr::ApplyIndirect {
                    dst: ArcVarId::new(3),
                    ty: Idx::INT,
                    closure: ArcVarId::new(1),
                    args: vec![ArcVarId::new(2)],
                    arg_ownership: vec![ArgOwnership::Borrowed],
                },
                ArcInstr::RcDec {
                    var: ArcVarId::new(1),
                    strategy: RcStrategy::Closure,
                    atomicity: RcAtomicity::Atomic,
                },
            ],
            terminator: ArcTerminator::Return {
                value: ArcVarId::new(3),
            },
        }],
        entry: ArcBlockId::new(0),
        var_types: vec![Idx::INT, closure_type, Idx::INT, Idx::INT],
        num_captures: 0,
        ..ArcFunction::default()
    };
    let lambda_function = ArcFunction {
        name: lambda,
        params: vec![
            ori_arc::ArcParam {
                var: ArcVarId::new(0),
                ty: Idx::INT,
                ownership: Ownership::Owned,
            },
            ori_arc::ArcParam {
                var: ArcVarId::new(1),
                ty: Idx::INT,
                ownership: Ownership::Borrowed,
            },
        ],
        return_type: Idx::INT,
        blocks: vec![ArcBlock {
            id: ArcBlockId::new(0),
            params: Vec::new(),
            body: Vec::new(),
            terminator: ArcTerminator::Return {
                value: ArcVarId::new(0),
            },
        }],
        entry: ArcBlockId::new(0),
        var_types: vec![Idx::INT, Idx::INT],
        num_captures: 1,
        ..ArcFunction::default()
    };
    let executable = close_test_executable(
        symbols,
        pool,
        vec![main_function, lambda_function],
        vec![FunctionFamilyTopology::new(main, vec![lambda])],
        main,
    );

    (executable, main, lambda)
}

/// Build a closed program whose closure borrows and returns a list.
pub(super) fn borrowed_list_closure_executable() -> ExecutableProgram {
    let symbols = SharedInterner::new();
    let main = symbols.intern("main");
    let lambda = symbols.intern("__lambda_main_borrowed_list");
    let mut pool = Pool::new();
    let list_type = pool.list(Idx::INT);
    let closure_type = pool.function(&[list_type], list_type);

    let main_function = ArcFunction {
        name: main,
        params: Vec::new(),
        return_type: list_type,
        blocks: vec![ArcBlock {
            id: ArcBlockId::new(0),
            params: Vec::new(),
            body: vec![
                ArcInstr::Let {
                    dst: ArcVarId::new(0),
                    ty: Idx::INT,
                    value: ArcValue::Literal(LitValue::Int(7)),
                },
                ArcInstr::Construct {
                    dst: ArcVarId::new(1),
                    ty: list_type,
                    ctor: CtorKind::ListLiteral,
                    args: vec![ArcVarId::new(0)],
                },
                ArcInstr::PartialApply {
                    dst: ArcVarId::new(2),
                    ty: closure_type,
                    func: lambda,
                    args: Vec::new(),
                },
                ArcInstr::ApplyIndirect {
                    dst: ArcVarId::new(3),
                    ty: list_type,
                    closure: ArcVarId::new(2),
                    args: vec![ArcVarId::new(1)],
                    arg_ownership: vec![ArgOwnership::Borrowed],
                },
                ArcInstr::RcDec {
                    var: ArcVarId::new(2),
                    strategy: RcStrategy::Closure,
                    atomicity: RcAtomicity::Atomic,
                },
                ArcInstr::RcDec {
                    var: ArcVarId::new(1),
                    strategy: RcStrategy::HeapPointer,
                    atomicity: RcAtomicity::Atomic,
                },
            ],
            terminator: ArcTerminator::Return {
                value: ArcVarId::new(3),
            },
        }],
        entry: ArcBlockId::new(0),
        var_types: vec![Idx::INT, list_type, closure_type, list_type],
        num_captures: 0,
        ..ArcFunction::default()
    };
    let lambda_function = ArcFunction {
        name: lambda,
        params: vec![ori_arc::ArcParam {
            var: ArcVarId::new(0),
            ty: list_type,
            ownership: Ownership::Borrowed,
        }],
        return_type: list_type,
        blocks: vec![ArcBlock {
            id: ArcBlockId::new(0),
            params: Vec::new(),
            body: Vec::new(),
            terminator: ArcTerminator::Return {
                value: ArcVarId::new(0),
            },
        }],
        entry: ArcBlockId::new(0),
        var_types: vec![list_type],
        num_captures: 0,
        ..ArcFunction::default()
    };

    close_test_executable(
        symbols,
        pool,
        vec![main_function, lambda_function],
        vec![FunctionFamilyTopology::new(main, vec![lambda])],
        main,
    )
}

/// Validate a test program with `main` as both root and CLI entry.
pub(super) fn close_test_executable(
    symbols: SharedInterner,
    pool: Pool,
    functions: Vec<ArcFunction>,
    function_families: Vec<FunctionFamilyTopology>,
    main: Name,
) -> ExecutableProgram {
    close_test_artifact(
        symbols,
        pool,
        functions,
        function_families,
        main,
        Some(main),
        Vec::new(),
    )
}

/// Realize function metadata and validate a configurable test artifact.
pub(super) fn close_test_artifact(
    symbols: SharedInterner,
    pool: Pool,
    mut functions: Vec<ArcFunction>,
    function_families: Vec<FunctionFamilyTopology>,
    root: Name,
    cli_entry: Option<Name>,
    externals: Vec<ExternalCallable>,
) -> ExecutableProgram {
    let classifier = ArcClassifier::new(&pool);
    for function in &mut functions {
        function.var_reprs = ori_arc::compute_var_reprs(function, &classifier, &pool);
        function.var_metadata_state = ori_arc::VariableMetadataState::RepresentationsReady;
        function.var_rc_strategies = compute_var_rc_strategies(function, &pool);
        function.var_metadata_state = ori_arc::VariableMetadataState::Realized;
    }

    let contracts: std::collections::HashMap<Name, MemoryContract> = functions
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
            let contract = contracts
                .get(&function.name)
                .unwrap_or_else(|| panic!("test contract should exist"));
            (function.name, contract.function_effect_facts(function))
        })
        .collect();
    let fresh_return_facts = functions
        .iter()
        .map(|function| {
            let contract = contracts
                .get(&function.name)
                .unwrap_or_else(|| panic!("test contract should exist"));
            (function.name, contract.fresh_self_allocation_facts())
        })
        .collect();
    let param_disjointness = functions
        .iter()
        .map(|function| {
            let param_types = function
                .params
                .iter()
                .map(|parameter| parameter.ty)
                .collect::<Vec<_>>();
            (function.name, prove_param_disjointness(&param_types, &pool))
        })
        .collect();
    let type_registry = TypeRegistry::new();
    let frozen_closure_adapters =
        ori_arc::freeze_closure_adapter_plans(&functions, &contracts, &pool, &type_registry)
            .unwrap_or_else(|errors| panic!("closure adapter facts should freeze: {errors:?}"));
    let externals = validate_external_callables(externals, &pool)
        .unwrap_or_else(|error| panic!("test external callables should validate: {error}"));

    ExecutableProgram::validate(ExecutableProgramParts {
        version: EXECUTABLE_PROGRAM_VERSION,
        symbols,
        pool,
        functions,
        function_families,
        contracts: contracts.into_iter().collect(),
        function_effects,
        fresh_return_facts,
        param_disjointness,
        callable_facts: frozen_closure_adapters.callable_facts,
        closure_adapters: frozen_closure_adapters.adapters,
        retain_plans: frozen_closure_adapters.retain_plans,
        roots: vec![root],
        cli_entry,
        externals,
        method_targets: FxHashMap::default(),
        user_drop_bindings: Vec::new(),
        repr_plan: ReprPlan::new(NarrowingPolicy::Disabled),
        type_registry,
    })
    .unwrap_or_else(|error| panic!("test executable should validate: {error}"))
}
