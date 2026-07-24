use ori_arc::{
    prove_param_disjointness, ArgOwnership, CtorKind, FrozenClosureAdapters, MemoryContract,
    RcAtomicity, RcStrategy, RetainPlanTable,
};
use ori_ir::{BinaryOp, Name, SharedInterner};
use ori_repr::executable::{
    CallableTarget, ExecutableProgram, ExecutableProgramParts, FunctionFamilyTopology, FunctionId,
    RuntimeCall, EXECUTABLE_PROGRAM_VERSION,
};
use ori_repr::{NarrowingPolicy, ReprPlan};
use ori_types::{Idx, Pool, TypeRegistry};
use rustc_hash::FxHashMap;

use crate::bytecode::{
    BytecodeFunction, BytecodeProgram, CallArgument, CallArgumentListId, Constant, Continuation,
    IntBinaryOp, MoveListId, Op, OperandListId, RcSemantics, RegisterClass, StringId,
    SwitchTableId, TableKind, VmTypeId,
};
use crate::{
    execute_physical_profiled, execute_physical_report, execute_profiled, execute_report, prepare,
    verify, ExecutionConfig, ExecutionReport, ExitValue, PhysicalLayoutViolation, PhysicalOptions,
    PrepareError,
};

use super::{
    main_function_id, minimal_arc_function, must_prepare_candidate, pc, register,
    verified_branch_program, verified_linear_program, PhysicalLane,
};

#[test]
fn identity_execution_matches_linear_immediates_and_copy_noop() {
    let program = verified_linear_program();

    assert_execution_parity(&program, ExecutionConfig::default());

    let report = physical_report(&program, ExecutionConfig::default());
    assert!(matches!(report.result, Ok(ExitValue::Int(42))));
}

#[test]
fn identity_execution_matches_branch_join_and_parallel_moves() {
    let program = verified_branch_program();

    assert_execution_parity(&program, ExecutionConfig::default());

    let report = physical_report(&program, ExecutionConfig::default());
    assert!(matches!(report.result, Ok(ExitValue::Int(7))));
}

#[test]
fn identity_execution_matches_loop_backedges_and_switches() {
    let loop_program = verified_loop_program();
    let switch_program = verified_switch_program();

    assert_execution_parity(&loop_program, ExecutionConfig::default());
    assert_execution_parity(&switch_program, ExecutionConfig::default());

    assert!(matches!(
        physical_report(&loop_program, ExecutionConfig::default()).result,
        Ok(ExitValue::Int(5))
    ));
    assert!(matches!(
        physical_report(&switch_program, ExecutionConfig::default()).result,
        Ok(ExitValue::Int(9))
    ));
}

#[test]
fn identity_execution_matches_direct_call_frames_and_returns() {
    let program = verified_direct_call_program();

    assert_execution_parity(&program, ExecutionConfig::default());

    let report = physical_report(&program, ExecutionConfig::default());
    assert!(matches!(report.result, Ok(ExitValue::Int(42))));
    assert_eq!(report.metrics.peak_frames, 2);
}

#[test]
fn identity_execution_matches_call_frame_quota_failure() {
    let program = verified_direct_call_program();
    let config = ExecutionConfig {
        max_frames: 1,
        ..ExecutionConfig::default()
    };

    assert_execution_parity(&program, config);

    assert!(matches!(
        physical_report(&program, config).result,
        Err(crate::ExecutionError::ResourceLimit {
            resource: "call frames",
            limit: 1,
        })
    ));
}

#[test]
fn identity_execution_preserves_runtime_ownership_and_rc_cleanup() {
    let program = verified_list_mutation_program();

    assert_execution_parity(&program, ExecutionConfig::default());

    let report = physical_report(&program, ExecutionConfig::default());
    assert!(matches!(report.result, Ok(ExitValue::Int(2))));
    assert_eq!(report.metrics.cumulative_heap_allocations, 2);
    assert_eq!(report.metrics.exit_live_heap_objects, 0);
    assert_eq!(report.metrics.final_live_heap_objects, 0);
}

#[test]
fn identity_execution_preserves_construct_operand_order() {
    let program = verified_construct_program();

    assert_execution_parity(&program, ExecutionConfig::default());

    assert!(matches!(
        physical_report(&program, ExecutionConfig::default()).result,
        Ok(ExitValue::Aggregate { variant: 0, ref fields })
            if fields == &[
                ExitValue::Bool(true),
                ExitValue::Int(7),
                ExitValue::Bool(true),
            ]
    ));
}

#[test]
fn identity_execution_matches_output_and_panic_unwind_prefix() {
    let program = verified_output_panic_program();

    assert_execution_parity(&program, ExecutionConfig::default());

    let report = physical_report(&program, ExecutionConfig::default());
    assert_eq!(report.output, b"physical unwind\n");
    assert!(matches!(
        report.result,
        Err(crate::ExecutionError::Panic { ref message }) if message == "physical unwind"
    ));
}

#[test]
fn identity_execution_matches_unreachable_failure() {
    let program = verified_complete(vec![Op::Unreachable], Vec::new(), TestTables::default());

    assert_execution_parity(&program, ExecutionConfig::default());

    assert!(matches!(
        physical_report(&program, ExecutionConfig::default()).result,
        Err(crate::ExecutionError::ReachedUnreachable)
    ));
}

#[test]
fn identity_execution_matches_every_step_quota_prefix() {
    let program = verified_loop_program();

    for max_steps in 0..24 {
        assert_execution_parity(
            &program,
            ExecutionConfig {
                max_steps,
                ..ExecutionConfig::default()
            },
        );
    }
}

#[test]
fn identity_execution_matches_canonical_probe_coordinates() {
    let program = verified_loop_program();
    let canonical = execute_profiled(&program, ExecutionConfig::default());
    let plan = prepare(&program, PhysicalOptions::default())
        .unwrap_or_else(|error| panic!("physical plan should prepare: {error}"));
    let physical = execute_physical_profiled(&plan, ExecutionConfig::default());

    assert_reports_equal(&canonical.execution, &physical.execution);
    assert_eq!(canonical.profile.dispatches, physical.profile.dispatches);
    assert_eq!(canonical.profile.opcodes, physical.profile.opcodes);
    assert_eq!(canonical.profile.all_pairs, physical.profile.all_pairs);
    assert_eq!(
        canonical.profile.linear_fallthrough_pairs,
        physical.profile.linear_fallthrough_pairs
    );
    assert_eq!(canonical.profile.regions, physical.profile.regions);
    assert_eq!(physical.profile.program().metrics(), program.metrics());
}

#[test]
fn malformed_candidate_is_rejected_before_branding() {
    let program = verified_linear_program();
    let mut candidate = must_prepare_candidate(&program, PhysicalOptions::default());
    let function = program.program.main.index();
    candidate.functions[function].writes[0] = PhysicalLane::new(1);

    let result = candidate.validate();

    assert!(matches!(
        result,
        Err(PrepareError::InvalidLayout {
            violation: PhysicalLayoutViolation::WriteBindingMismatch { .. },
            ..
        })
    ));
}

fn assert_execution_parity(program: &crate::VerifiedProgram, config: ExecutionConfig) {
    let canonical = execute_report(program, config);
    let physical = physical_report(program, config);
    assert_reports_equal(&canonical, &physical);
}

fn physical_report(program: &crate::VerifiedProgram, config: ExecutionConfig) -> ExecutionReport {
    let plan = prepare(program, PhysicalOptions::default())
        .unwrap_or_else(|error| panic!("physical plan should prepare: {error}"));
    execute_physical_report(&plan, config)
}

fn assert_reports_equal(canonical: &ExecutionReport, physical: &ExecutionReport) {
    match (&canonical.result, &physical.result) {
        (Ok(canonical), Ok(physical)) => assert_eq!(canonical, physical),
        (Err(canonical), Err(physical)) => {
            assert_eq!(
                std::mem::discriminant(canonical),
                std::mem::discriminant(physical)
            );
            assert_eq!(format!("{canonical:?}"), format!("{physical:?}"));
            assert_eq!(canonical.to_string(), physical.to_string());
        }
        (canonical, physical) => {
            panic!("execution modes disagreed: canonical={canonical:?}, physical={physical:?}")
        }
    }
    assert_eq!(canonical.output, physical.output);
    assert_eq!(canonical.metrics, physical.metrics);
}

fn verified_loop_program() -> crate::VerifiedProgram {
    verified_complete(
        vec![
            Op::Const {
                dst: register(0),
                value: Constant::Int(0),
            },
            Op::Const {
                dst: register(1),
                value: Constant::Int(5),
            },
            Op::IntBinary {
                dst: register(2),
                op: int_binary(BinaryOp::Lt),
                lhs: register(0),
                rhs: register(1),
            },
            Op::Branch {
                cond: register(2),
                then_pc: pc(4),
                else_pc: pc(7),
            },
            Op::Const {
                dst: register(3),
                value: Constant::Int(1),
            },
            Op::IntBinary {
                dst: register(0),
                op: int_binary(BinaryOp::Add),
                lhs: register(0),
                rhs: register(3),
            },
            Op::Jump {
                target: pc(2),
                moves: move_id(0),
            },
            Op::Return { value: register(0) },
        ],
        vec![
            RegisterClass::Int,
            RegisterClass::Int,
            RegisterClass::Bool,
            RegisterClass::Int,
        ],
        TestTables {
            moves: vec![Box::default()],
            ..TestTables::default()
        },
    )
}

fn verified_switch_program() -> crate::VerifiedProgram {
    verified_complete(
        vec![
            Op::Const {
                dst: register(0),
                value: Constant::Int(1),
            },
            Op::Switch {
                scrutinee: register(0),
                table: switch_id(0),
                default_pc: pc(4),
            },
            Op::Const {
                dst: register(1),
                value: Constant::Int(9),
            },
            Op::Jump {
                target: pc(5),
                moves: move_id(0),
            },
            Op::Const {
                dst: register(2),
                value: Constant::Int(7),
            },
            Op::Return { value: register(3) },
        ],
        vec![
            RegisterClass::Int,
            RegisterClass::Int,
            RegisterClass::Int,
            RegisterClass::Int,
        ],
        TestTables {
            moves: vec![vec![(register(3), register(1))].into_boxed_slice()],
            switches: vec![vec![(1, pc(2))].into_boxed_slice()],
            ..TestTables::default()
        },
    )
}

fn verified_construct_program() -> crate::VerifiedProgram {
    verified_complete(
        vec![
            Op::Const {
                dst: register(0),
                value: Constant::Bool(true),
            },
            Op::Const {
                dst: register(1),
                value: Constant::Int(7),
            },
            Op::Construct {
                dst: register(2),
                ctor: CtorKind::Tuple,
                args: operand_id(0),
            },
            Op::Return { value: register(2) },
        ],
        vec![
            RegisterClass::Bool,
            RegisterClass::Int,
            RegisterClass::Other,
        ],
        TestTables {
            operands: vec![vec![register(0), register(1), register(0)].into_boxed_slice()],
            ..TestTables::default()
        },
    )
}

fn verified_list_mutation_program() -> crate::VerifiedProgram {
    let heap_rc = RcSemantics::new(RcStrategy::HeapPointer, RcAtomicity::default_atomic());
    verified_complete(
        vec![
            Op::Const {
                dst: register(0),
                value: Constant::Int(7),
            },
            Op::Construct {
                dst: register(1),
                ctor: CtorKind::ListLiteral,
                args: operand_id(0),
            },
            Op::Const {
                dst: register(2),
                value: Constant::Int(9),
            },
            Op::Call {
                dst: register(3),
                callee: CallableTarget::Runtime(RuntimeCall::ListPush),
                args: call_id(0),
                normal: Continuation::Next,
                unwind: None,
            },
            Op::Call {
                dst: register(4),
                callee: CallableTarget::Runtime(RuntimeCall::Length),
                args: call_id(1),
                normal: Continuation::Next,
                unwind: None,
            },
            Op::RcDec {
                var: register(3),
                semantics: heap_rc,
            },
            Op::RcDec {
                var: register(1),
                semantics: heap_rc,
            },
            Op::Return { value: register(4) },
        ],
        vec![
            RegisterClass::Int,
            RegisterClass::Other,
            RegisterClass::Int,
            RegisterClass::Other,
            RegisterClass::Int,
        ],
        TestTables {
            call_arguments: vec![
                vec![
                    CallArgument::new(register(1), ArgOwnership::Borrowed),
                    CallArgument::new(register(2), ArgOwnership::Borrowed),
                ]
                .into_boxed_slice(),
                vec![CallArgument::new(register(3), ArgOwnership::Borrowed)].into_boxed_slice(),
            ],
            operands: vec![vec![register(0)].into_boxed_slice()],
            rc_strategies: Some(vec![
                None,
                Some(RcStrategy::HeapPointer),
                None,
                Some(RcStrategy::HeapPointer),
                None,
            ]),
            ..TestTables::default()
        },
    )
}

fn verified_output_panic_program() -> crate::VerifiedProgram {
    verified_complete(
        vec![
            Op::Const {
                dst: register(0),
                value: Constant::String(string_id(0)),
            },
            Op::Call {
                dst: register(1),
                callee: CallableTarget::Runtime(RuntimeCall::Print),
                args: call_id(0),
                normal: Continuation::Next,
                unwind: None,
            },
            Op::Call {
                dst: register(1),
                callee: CallableTarget::Runtime(RuntimeCall::Panic),
                args: call_id(1),
                normal: Continuation::At(pc(3)),
                unwind: Some(pc(4)),
            },
            Op::Return { value: register(1) },
            Op::Resume,
        ],
        vec![RegisterClass::String, RegisterClass::Other],
        TestTables {
            call_arguments: vec![
                vec![CallArgument::new(register(0), ArgOwnership::Borrowed)].into_boxed_slice(),
                vec![CallArgument::new(register(0), ArgOwnership::Borrowed)].into_boxed_slice(),
            ],
            strings: vec!["physical unwind".to_owned()],
            ..TestTables::default()
        },
    )
}

fn verified_direct_call_program() -> crate::VerifiedProgram {
    let fixture = direct_call_artifact();
    let main = fixture
        .executable
        .cli_entry()
        .unwrap_or_else(|| panic!("fixture must have a CLI entry"));
    let double = fixture
        .executable
        .function_id(fixture.double_name)
        .unwrap_or_else(|| panic!("callee should have a function identity"));
    let mut functions = vec![
        (main, direct_call_main_function(fixture.main_name, double)),
        (double, direct_call_double_function(fixture.double_name)),
    ];
    functions.sort_by_key(|(identity, _)| *identity);

    verify(BytecodeProgram::new(
        functions
            .into_iter()
            .map(|(_, function)| function)
            .collect(),
        vec![vec![CallArgument::new(register(0), ArgOwnership::Borrowed)].into_boxed_slice()],
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
        main,
    ))
    .unwrap_or_else(|error| panic!("direct-call bytecode should verify: {error}"))
}

struct DirectCallArtifact {
    executable: ExecutableProgram,
    main_name: Name,
    double_name: Name,
}

fn direct_call_artifact() -> DirectCallArtifact {
    let symbols = SharedInterner::new();
    let main_name = symbols.intern("a_main");
    let double_name = symbols.intern("z_double");
    let functions = vec![
        minimal_arc_function(main_name),
        minimal_arc_function(double_name),
    ];
    let contracts = [main_name, double_name]
        .into_iter()
        .map(|name| (name, MemoryContract::conservative(0)))
        .collect();
    let function_effects = functions
        .iter()
        .map(|function| {
            let contract = MemoryContract::conservative(0);
            (function.name, contract.function_effect_facts(function))
        })
        .collect();
    let fresh_return_facts = functions
        .iter()
        .map(|function| {
            let contract = MemoryContract::conservative(0);
            (function.name, contract.fresh_self_allocation_facts())
        })
        .collect();
    let pool = Pool::new();
    let param_disjointness = functions
        .iter()
        .map(|function| (function.name, prove_param_disjointness(&[], &pool)))
        .collect();
    let callable_facts = ori_arc::freeze_function_callable_facts(&functions, &pool);
    let executable = ExecutableProgram::validate(ExecutableProgramParts {
        version: EXECUTABLE_PROGRAM_VERSION,
        symbols,
        pool,
        functions,
        function_families: vec![
            FunctionFamilyTopology::new(main_name, Vec::new()),
            FunctionFamilyTopology::new(double_name, Vec::new()),
        ],
        contracts,
        function_effects,
        fresh_return_facts,
        param_disjointness,
        callable_facts,
        closure_adapters: FrozenClosureAdapters::default().adapters,
        retain_plans: RetainPlanTable::default(),
        roots: vec![main_name],
        cli_entry: Some(main_name),
        externals: ori_repr::executable::ValidatedExternalCallables::empty(),
        method_targets: FxHashMap::default(),
        user_drop_bindings: Vec::new(),
        repr_plan: ReprPlan::new(NarrowingPolicy::Disabled),
        type_registry: TypeRegistry::new(),
    })
    .unwrap_or_else(|error| panic!("function identities should validate: {error}"));

    DirectCallArtifact {
        executable,
        main_name,
        double_name,
    }
}

fn direct_call_main_function(name: Name, double: FunctionId) -> BytecodeFunction {
    BytecodeFunction {
        name,
        params: Box::default(),
        return_type: VmTypeId::from_raw(Idx::INT.raw()),
        param_ownership: Box::default(),
        capture_count: 0,
        closure_adapter: None,
        ops: vec![
            Op::Const {
                dst: register(0),
                value: Constant::Int(21),
            },
            Op::Call {
                dst: register(1),
                callee: CallableTarget::Function(double),
                args: call_id(0),
                normal: Continuation::Next,
                unwind: None,
            },
            Op::Return { value: register(1) },
        ]
        .into_boxed_slice(),
        entry: pc(0),
        register_count: 2,
        register_types: vec![VmTypeId::from_raw(Idx::INT.raw()); 2].into_boxed_slice(),
        register_classes: vec![RegisterClass::Int; 2].into_boxed_slice(),
        register_rc_strategies: vec![None; 2].into_boxed_slice(),
        register_closure_signatures: vec![None; 2].into_boxed_slice(),
    }
}

fn direct_call_double_function(name: Name) -> BytecodeFunction {
    BytecodeFunction {
        name,
        params: vec![register(0)].into_boxed_slice(),
        return_type: VmTypeId::from_raw(Idx::INT.raw()),
        param_ownership: vec![ori_arc::Ownership::Borrowed].into_boxed_slice(),
        capture_count: 0,
        closure_adapter: None,
        ops: vec![
            Op::Const {
                dst: register(1),
                value: Constant::Int(2),
            },
            Op::IntBinary {
                dst: register(2),
                op: int_binary(BinaryOp::Mul),
                lhs: register(0),
                rhs: register(1),
            },
            Op::Return { value: register(2) },
        ]
        .into_boxed_slice(),
        entry: pc(0),
        register_count: 3,
        register_types: vec![VmTypeId::from_raw(Idx::INT.raw()); 3].into_boxed_slice(),
        register_classes: vec![RegisterClass::Int; 3].into_boxed_slice(),
        register_rc_strategies: vec![None; 3].into_boxed_slice(),
        register_closure_signatures: vec![None; 3].into_boxed_slice(),
    }
}

fn verified_complete(
    operations: Vec<Op>,
    register_classes: Vec<RegisterClass>,
    tables: TestTables,
) -> crate::VerifiedProgram {
    let register_count = register_classes.len();
    let function = BytecodeFunction {
        name: Name::EMPTY,
        params: Box::default(),
        return_type: VmTypeId::from_raw(Idx::INT.raw()),
        param_ownership: Box::default(),
        capture_count: 0,
        closure_adapter: None,
        ops: operations.into_boxed_slice(),
        entry: pc(0),
        register_count,
        register_types: vec![VmTypeId::from_raw(Idx::INT.raw()); register_count].into_boxed_slice(),
        register_classes: register_classes.into_boxed_slice(),
        register_rc_strategies: tables
            .rc_strategies
            .unwrap_or_else(|| vec![None; register_count])
            .into_boxed_slice(),
        register_closure_signatures: vec![None; register_count].into_boxed_slice(),
    };
    verify(BytecodeProgram::new(
        vec![function],
        tables.call_arguments,
        tables.operands,
        tables.moves,
        tables.switches,
        tables.strings,
        main_function_id(),
    ))
    .unwrap_or_else(|error| panic!("test bytecode should verify: {error}"))
}

#[derive(Default)]
struct TestTables {
    call_arguments: Vec<Box<[CallArgument]>>,
    operands: Vec<Box<[crate::bytecode::Register]>>,
    moves: Vec<Box<[(crate::bytecode::Register, crate::bytecode::Register)]>>,
    switches: Vec<Box<[(u64, crate::bytecode::Pc)]>>,
    strings: Vec<String>,
    rc_strategies: Option<Vec<Option<RcStrategy>>>,
}

fn int_binary(operation: BinaryOp) -> IntBinaryOp {
    IntBinaryOp::from_binary(operation)
        .unwrap_or_else(|| panic!("test operation should have an integer opcode"))
}

fn call_id(index: usize) -> CallArgumentListId {
    CallArgumentListId::new(index, TableKind::CallArguments)
        .unwrap_or_else(|error| panic!("test call table should fit: {error}"))
}

fn operand_id(index: usize) -> OperandListId {
    OperandListId::new(index, TableKind::Operands)
        .unwrap_or_else(|error| panic!("test operand table should fit: {error}"))
}

fn move_id(index: usize) -> MoveListId {
    MoveListId::new(index, TableKind::Moves)
        .unwrap_or_else(|error| panic!("test move table should fit: {error}"))
}

fn switch_id(index: usize) -> SwitchTableId {
    SwitchTableId::new(index, TableKind::Switches)
        .unwrap_or_else(|error| panic!("test switch table should fit: {error}"))
}

fn string_id(index: usize) -> StringId {
    StringId::new(index, TableKind::Strings)
        .unwrap_or_else(|error| panic!("test string table should fit: {error}"))
}
