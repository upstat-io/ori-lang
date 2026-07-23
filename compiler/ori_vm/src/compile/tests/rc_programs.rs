use super::*;

/// Return the first RC opcode location and mutable semantics record.
pub(super) fn first_rc_semantics_mut(
    bytecode: &mut BytecodeProgram,
) -> (usize, &mut crate::bytecode::RcSemantics) {
    for (pc, operation) in bytecode.functions[0].ops.iter_mut().enumerate() {
        match operation {
            Op::RcInc { semantics, .. } | Op::RcDec { semantics, .. } => {
                return (pc, semantics);
            }
            _ => {}
        }
    }
    panic!("test bytecode should contain an RC operation")
}

/// Construct the first register outside `register_count`.
pub(super) fn invalid_register(register_count: usize) -> Register {
    let raw = u32::try_from(register_count)
        .unwrap_or_else(|_| panic!("test register count should fit in bytecode identity"));
    Register::from_arc(ArcVarId::new(raw))
}

/// Compile an RC fixture or fail the test with the compiler error.
pub(super) fn must_compile(strategy: RcStrategy, atomicity: RcAtomicity) -> BytecodeProgram {
    let (_, result) = compile_result(strategy, atomicity);
    match result {
        Ok(bytecode) => bytecode,
        Err(error) => panic!("test executable should compile: {error}"),
    }
}

/// Compile a fixture whose value and emitted RC operation use `strategy`.
pub(super) fn compile_result(
    strategy: RcStrategy,
    atomicity: RcAtomicity,
) -> (Name, Result<BytecodeProgram, CompileError>) {
    compile_result_for_value(strategy, strategy, atomicity)
}

/// Compile an RC fixture with independently selected value and operation strategies.
pub(super) fn compile_result_for_value(
    value_strategy: RcStrategy,
    strategy: RcStrategy,
    atomicity: RcAtomicity,
) -> (Name, Result<BytecodeProgram, CompileError>) {
    let symbols = SharedInterner::new();
    let main = symbols.intern("main");
    let message = symbols.intern("rc semantics");
    let iter = symbols.intern("iter");
    let option_name = symbols.intern("Option");
    let mut pool = Pool::new();
    let RcValueFixture {
        mut body,
        rc_var,
        var_types,
        method_call_facts,
    } = rc_value_fixture(value_strategy, &mut pool, message, iter, option_name);
    body.extend([
        ArcInstr::RcInc {
            var: rc_var,
            count: 2,
            strategy,
            atomicity,
        },
        ArcInstr::RcDec {
            var: rc_var,
            strategy,
            atomicity,
        },
    ]);
    let mut function = ArcFunction {
        name: main,
        params: Vec::new(),
        return_type: Idx::INT,
        blocks: vec![ArcBlock {
            id: ArcBlockId::new(0),
            params: Vec::new(),
            body,
            terminator: ArcTerminator::Return {
                value: ArcVarId::new(0),
            },
        }],
        entry: ArcBlockId::new(0),
        var_types,
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
        method_call_facts,
        operator_call_facts: Vec::new(),
        direct_call_facts: Vec::new(),
        yield_allocations: Vec::new(),
        class_ledger_emission: false,
    };
    let classifier = ArcClassifier::new(&pool);
    function.var_reprs = ori_arc::compute_var_reprs(&function, &classifier, &pool);
    function.var_metadata_state = ori_arc::VariableMetadataState::RepresentationsReady;
    function.var_rc_strategies = compute_var_rc_strategies(&function, &pool);
    function.var_metadata_state = ori_arc::VariableMetadataState::Realized;
    let contract = MemoryContract::conservative(function.params.len());

    let executable = validate_rc_value_executable(symbols, pool, function, main, contract);

    (main, compile(&executable))
}

/// Validate one RC fixture as a closed executable program.
pub(super) fn validate_rc_value_executable(
    symbols: SharedInterner,
    pool: Pool,
    function: ArcFunction,
    main: Name,
    contract: MemoryContract,
) -> ExecutableProgram {
    let function_effects = [(main, contract.function_effect_facts(&function))]
        .into_iter()
        .collect();
    let fresh_return_facts = [(main, contract.fresh_self_allocation_facts())]
        .into_iter()
        .collect();
    let param_disjointness = [(main, prove_param_disjointness(&[], &pool))]
        .into_iter()
        .collect();
    let contracts = [(main, contract)].into_iter().collect();
    let functions = vec![function];
    let callable_facts = ori_arc::freeze_function_callable_facts(&functions, &pool);
    match ExecutableProgram::validate(ExecutableProgramParts {
        version: EXECUTABLE_PROGRAM_VERSION,
        symbols,
        pool,
        functions,
        function_families: vec![FunctionFamilyTopology::new(main, Vec::new())],
        contracts,
        function_effects,
        fresh_return_facts,
        param_disjointness,
        callable_facts,
        closure_adapters: FrozenClosureAdapters::default().adapters,
        retain_plans: RetainPlanTable::default(),
        roots: vec![main],
        cli_entry: Some(main),
        externals: ori_repr::executable::ValidatedExternalCallables::empty(),
        method_targets: FxHashMap::default(),
        user_drop_bindings: Vec::new(),
        repr_plan: ReprPlan::new(NarrowingPolicy::Disabled),
        type_registry: TypeRegistry::new(),
    }) {
        Ok(program) => program,
        Err(error) => panic!("test executable should validate: {error}"),
    }
}
