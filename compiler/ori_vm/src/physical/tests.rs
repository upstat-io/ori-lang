use ori_arc::ir::PrimitiveFacts;
use ori_arc::uniqueness::{CowAnnotations, DropHints};
use ori_arc::{
    prove_param_disjointness, ArcBlock, ArcBlockId, ArcFunction, ArcTerminator, ArcValue, ArcVarId,
    ArgOwnership, FrozenClosureAdapters, LitValue, MemoryContract, RetainPlanTable,
};
use ori_ir::{BinaryOp, Name, SharedInterner};
use ori_repr::executable::{
    CallableTarget, ExecutableProgram, ExecutableProgramParts, FunctionFamilyTopology, FunctionId,
    RuntimeCall, EXECUTABLE_PROGRAM_VERSION,
};
use ori_repr::{NarrowingPolicy, ReprPlan};
use ori_types::{Idx, Pool, TypeRegistry};
use rustc_hash::FxHashMap;
use std::mem::size_of;

use crate::bytecode::{
    BytecodeFunction, BytecodeProgram, CallArgument, CallArgumentListId, Constant, Continuation,
    IntBinaryOp, MoveListId, Op, Pc, Register, RegisterClass, TableKind, VmTypeId,
};
use crate::verify;

use super::{
    prepare, prepare_candidate, PhysicalElementSizes, PhysicalFunctionPlan,
    PhysicalFunctionStorageMetrics, PhysicalImmediate, PhysicalOptions, PhysicalPlanMetrics,
    PhysicalRead, PhysicalVmPlanCandidate,
};
use crate::physical::layout::{PhysicalOpPlan, PhysicalPcPlan};
use crate::physical::PhysicalLane;

mod execution;
mod invariants;
mod planning;

#[test]
fn immediate_bindings_and_same_binding_copy_are_explicit_plan_facts() {
    let program = verified_linear_program();

    let plan = must_prepare(&program, PhysicalOptions::default());

    assert_eq!(plan.metrics().logical_lanes, 4);
    assert_eq!(plan.metrics().physical_lanes, 4);
    assert_eq!(plan.metrics().canonical_ops, 6);
    assert_eq!(plan.metrics().physical_ops, 6);
    assert_eq!(plan.metrics().read_bindings, 5);
    assert_eq!(plan.metrics().write_bindings, 5);
    assert_eq!(plan.metrics().immediate_bindings, 3);
    assert_eq!(plan.metrics().coalesced_copies, 1);
    assert!(plan.metrics().owned_plan_bytes > 0);
    assert_eq!(plan.program().metrics(), program.metrics());
    assert_eq!(plan.options(), PhysicalOptions::default());
    let function = must_execution_view(&plan).function_at(program.program.main.index());
    assert_eq!(function.lane_count(), 4);
    assert_eq!(function.lane_at(3).index(), 3);
    assert_eq!(
        function.canonical_pc_at(1).reads(),
        [PhysicalRead::Immediate(PhysicalImmediate::Int(40))]
    );
    assert!(function.canonical_pc_at(4).copy_is_noop());
}

#[test]
fn storage_metrics_account_for_every_exact_boxed_table_payload() {
    let program = verified_linear_program();
    let plan = must_prepare(&program, PhysicalOptions::default());
    let sizes = plan.element_sizes();
    let function = plan
        .function_storage_metrics(program.program.main.index())
        .unwrap_or_else(|error| {
            panic!("main function must have physical storage metrics: {error}")
        });

    assert_element_sizes(sizes);
    assert_function_storage(sizes, function);
    assert_plan_storage(plan.metrics(), function);
}

fn assert_element_sizes(sizes: PhysicalElementSizes) {
    assert_eq!(sizes.canonical_op, size_of::<Op>());
    assert_eq!(
        sizes.physical_function_plan,
        size_of::<PhysicalFunctionPlan>()
    );
    assert_eq!(sizes.physical_op_plan, size_of::<PhysicalOpPlan>());
    assert_eq!(sizes.physical_pc_plan, size_of::<PhysicalPcPlan>());
    assert_eq!(sizes.physical_read, size_of::<PhysicalRead>());
    assert_eq!(sizes.physical_write, size_of::<PhysicalLane>());
    assert_eq!(sizes.physical_lane, size_of::<PhysicalLane>());
}

fn assert_function_storage(sizes: PhysicalElementSizes, function: PhysicalFunctionStorageMetrics) {
    assert_eq!(
        function.canonical_op_bytes,
        sizes.canonical_op * function.canonical_ops
    );
    assert_eq!(
        function.physical_op_bytes,
        sizes.physical_op_plan * function.physical_ops
    );
    assert_eq!(
        function.physical_pc_bytes,
        sizes.physical_pc_plan * function.physical_pcs
    );
    assert_eq!(
        function.physical_read_bytes,
        sizes.physical_read * function.reads
    );
    assert_eq!(
        function.physical_write_bytes,
        sizes.physical_write * function.writes
    );
    assert_eq!(
        function.physical_lane_bytes,
        sizes.physical_lane * function.lanes
    );
    assert_eq!(
        function.retained_canonical_and_plan_bytes,
        function.canonical_op_bytes + function.owned_plan_bytes
    );
}

fn assert_plan_storage(metrics: PhysicalPlanMetrics, function: PhysicalFunctionStorageMetrics) {
    assert_eq!(metrics.owned_plan_bytes, function.owned_plan_bytes);
    assert_eq!(metrics.canonical_op_bytes, function.canonical_op_bytes);
    assert_eq!(
        metrics.retained_canonical_and_plan_bytes,
        function.retained_canonical_and_plan_bytes
    );
    assert_eq!(metrics.planning_scratch_current_payload_bytes, 0);
    assert!(metrics.planning_scratch_peak_payload_bytes_lower_bound > 0);
    assert!(metrics.planning_scratch_cumulative_allocation_bytes_lower_bound > 0);
    assert_eq!(metrics.validation_scratch_current_payload_bytes, 0);
    assert!(metrics.validation_scratch_peak_payload_bytes_lower_bound > 0);
    assert!(metrics.validation_scratch_cumulative_allocation_bytes_lower_bound > 0);
    assert_eq!(
        metrics.planning_scratch_peak_payload_bytes_lower_bound,
        metrics.validation_scratch_peak_payload_bytes_lower_bound
    );
    assert_eq!(
        metrics.planning_scratch_cumulative_allocation_bytes_lower_bound,
        metrics.validation_scratch_cumulative_allocation_bytes_lower_bound
    );
}

#[test]
fn storage_accounting_reports_overflow_instead_of_saturating() {
    assert!(matches!(
        super::metrics::checked_metric_add(usize::MAX, 1, "test count"),
        Err(super::PrepareError::MetricOverflow {
            metric: "test count"
        })
    ));
    assert!(matches!(
        super::metrics::checked_storage_mul::<PhysicalRead>(usize::MAX, 7, "test reads"),
        Err(super::PrepareError::FunctionStorageSizeOverflow {
            function: 7,
            table: "test reads"
        })
    ));
    assert!(matches!(
        super::metrics::checked_storage_add(usize::MAX, 1, 9, "test subtotal"),
        Err(super::PrepareError::FunctionStorageSizeOverflow {
            function: 9,
            table: "test subtotal"
        })
    ));
}

#[test]
fn storage_metrics_invalid_function_reports_typed_index_error() {
    let program = verified_linear_program();
    let plan = must_prepare(&program, PhysicalOptions::default());

    assert!(matches!(
        plan.function_storage_metrics(plan.function_count()),
        Err(super::PrepareError::FunctionIndexOutOfBounds { .. })
    ));
}

#[test]
fn disabled_preferences_preserve_identity_lanes_without_sparse_bindings() {
    let program = verified_linear_program();
    let options = PhysicalOptions {
        bind_immediates: false,
        coalesce_copies: false,
    };

    let plan = must_prepare(&program, options);

    assert_eq!(plan.metrics().logical_lanes, 4);
    assert_eq!(plan.metrics().physical_lanes, 4);
    assert_eq!(plan.metrics().immediate_bindings, 0);
    assert_eq!(plan.metrics().coalesced_copies, 0);
    let function = must_execution_view(&plan).function_at(program.program.main.index());
    assert_eq!(
        function.canonical_pc_at(1).reads(),
        [PhysicalRead::Lane(function.lane_at(0))]
    );
    assert!(!function.canonical_pc_at(4).copy_is_noop());
}

#[test]
fn control_flow_join_drops_disagreeing_immediate_bindings() {
    let program = verified_branch_program();

    let plan = must_prepare(&program, PhysicalOptions::default());
    let function = must_execution_view(&plan).function_at(program.program.main.index());

    assert_eq!(plan.metrics().immediate_bindings, 3);
    assert_eq!(
        function.canonical_pc_at(1).reads(),
        [PhysicalRead::Immediate(PhysicalImmediate::Bool(true))]
    );
    assert_eq!(
        function.canonical_pc_at(6).reads(),
        [PhysicalRead::Lane(function.lane_at(2))]
    );
}

#[test]
fn parallel_moves_propagate_immediates_from_one_pre_move_snapshot() {
    let moves = MoveListId::new(0, TableKind::Moves)
        .unwrap_or_else(|error| panic!("test move table should fit: {error}"));
    let program = verified_single_function_with_tables(
        vec![
            Op::Const {
                dst: register(0),
                value: Constant::Int(7),
            },
            Op::Const {
                dst: register(1),
                value: Constant::Int(9),
            },
            Op::Jump {
                target: pc(3),
                moves,
            },
            Op::IntBinary {
                dst: register(2),
                op: IntBinaryOp::from_binary(BinaryOp::Add)
                    .unwrap_or_else(|| panic!("integer addition should have a typed opcode")),
                lhs: register(0),
                rhs: register(1),
            },
            Op::Return { value: register(2) },
        ],
        vec![RegisterClass::Int, RegisterClass::Int, RegisterClass::Int],
        Vec::new(),
        vec![vec![(register(0), register(1)), (register(1), register(0))].into_boxed_slice()],
    );

    let plan = must_prepare(&program, PhysicalOptions::default());
    let function = must_execution_view(&plan).function_at(program.program.main.index());

    assert_eq!(
        function.canonical_pc_at(3).reads(),
        [
            PhysicalRead::Immediate(PhysicalImmediate::Int(9)),
            PhysicalRead::Immediate(PhysicalImmediate::Int(7)),
        ]
    );
}

#[test]
fn loop_backedge_invalidates_conflicting_first_iteration_immediate() {
    let no_moves = MoveListId::new(0, TableKind::Moves)
        .unwrap_or_else(|error| panic!("test move table should fit: {error}"));
    let program = verified_single_function_with_tables(
        vec![
            Op::Const {
                dst: register(0),
                value: Constant::Int(1),
            },
            Op::Jump {
                target: pc(2),
                moves: no_moves,
            },
            Op::Copy {
                dst: register(1),
                src: register(0),
            },
            Op::Const {
                dst: register(0),
                value: Constant::Int(2),
            },
            Op::Jump {
                target: pc(2),
                moves: no_moves,
            },
        ],
        vec![RegisterClass::Int, RegisterClass::Int],
        Vec::new(),
        vec![Box::default()],
    );

    let plan = must_prepare(&program, PhysicalOptions::default());
    let function = must_execution_view(&plan).function_at(program.program.main.index());

    assert_eq!(
        function.canonical_pc_at(2).reads(),
        [PhysicalRead::Lane(function.lane_at(0))]
    );
}

#[test]
fn call_normal_definition_and_unwind_preservation_remain_distinct() {
    let args = CallArgumentListId::new(0, TableKind::CallArguments)
        .unwrap_or_else(|error| panic!("test call-argument table should fit: {error}"));
    let program = verified_single_function_with_call_arguments(
        vec![
            Op::Const {
                dst: register(0),
                value: Constant::Int(7),
            },
            Op::Call {
                dst: register(1),
                callee: CallableTarget::Runtime(RuntimeCall::StringContains),
                args,
                normal: Continuation::At(pc(2)),
                unwind: Some(pc(3)),
            },
            Op::Return { value: register(1) },
            Op::Return { value: register(0) },
        ],
        vec![RegisterClass::Int, RegisterClass::Other],
        vec![vec![
            CallArgument::new(register(0), ArgOwnership::Borrowed),
            CallArgument::new(register(0), ArgOwnership::Owned),
        ]
        .into_boxed_slice()],
        Vec::new(),
        Vec::new(),
    );

    let plan = must_prepare(&program, PhysicalOptions::default());
    let function = must_execution_view(&plan).function_at(program.program.main.index());

    assert_eq!(
        function.canonical_pc_at(1).reads(),
        [
            PhysicalRead::Immediate(PhysicalImmediate::Int(7)),
            PhysicalRead::Immediate(PhysicalImmediate::Int(7)),
        ]
    );
    assert_eq!(
        function.canonical_pc_at(2).reads(),
        [PhysicalRead::Lane(function.lane_at(1))]
    );
    assert_eq!(
        function.canonical_pc_at(3).reads(),
        [PhysicalRead::Immediate(PhysicalImmediate::Int(7))]
    );
    assert_eq!(
        program.program.call_arguments[0]
            .iter()
            .map(|argument| argument.ownership())
            .collect::<Vec<_>>(),
        [ArgOwnership::Borrowed, ArgOwnership::Owned]
    );
}

#[test]
fn concrete_definition_proves_an_immediate_without_trusting_register_class() {
    let program = verified_single_function(
        vec![
            Op::Const {
                dst: register(0),
                value: Constant::Int(7),
            },
            Op::Return { value: register(0) },
        ],
        vec![RegisterClass::Other],
        Vec::new(),
    );

    let plan = must_prepare(&program, PhysicalOptions::default());
    let function = must_execution_view(&plan).function_at(program.program.main.index());

    assert_eq!(plan.metrics().immediate_bindings, 1);
    assert_eq!(
        function.canonical_pc_at(1).reads(),
        [PhysicalRead::Immediate(PhysicalImmediate::Int(7))]
    );
}

fn verified_linear_program() -> crate::VerifiedProgram {
    let operations = vec![
        Op::Const {
            dst: register(0),
            value: Constant::Int(40),
        },
        Op::Copy {
            dst: register(1),
            src: register(0),
        },
        Op::Const {
            dst: register(2),
            value: Constant::Int(2),
        },
        Op::IntBinary {
            dst: register(3),
            op: IntBinaryOp::from_binary(BinaryOp::Add)
                .unwrap_or_else(|| panic!("integer addition should have a typed opcode")),
            lhs: register(1),
            rhs: register(2),
        },
        Op::Copy {
            dst: register(3),
            src: register(3),
        },
        Op::Return { value: register(3) },
    ];
    verified_single_function(
        operations,
        vec![
            RegisterClass::Int,
            RegisterClass::Int,
            RegisterClass::Int,
            RegisterClass::Int,
        ],
        Vec::new(),
    )
}

fn verified_branch_program() -> crate::VerifiedProgram {
    let moves = MoveListId::new(0, TableKind::Moves)
        .unwrap_or_else(|error| panic!("test move table should fit: {error}"));
    let operations = vec![
        Op::Const {
            dst: register(0),
            value: Constant::Bool(true),
        },
        Op::Branch {
            cond: register(0),
            then_pc: pc(2),
            else_pc: pc(4),
        },
        Op::Const {
            dst: register(1),
            value: Constant::Int(7),
        },
        Op::Jump {
            target: pc(6),
            moves,
        },
        Op::Const {
            dst: register(1),
            value: Constant::Int(9),
        },
        Op::Jump {
            target: pc(6),
            moves,
        },
        Op::Return { value: register(2) },
    ];
    verified_single_function(
        operations,
        vec![RegisterClass::Bool, RegisterClass::Int, RegisterClass::Int],
        vec![vec![(register(2), register(1))].into_boxed_slice()],
    )
}

fn verified_single_function(
    operations: Vec<Op>,
    register_classes: Vec<RegisterClass>,
    moves: Vec<Box<[(Register, Register)]>>,
) -> crate::VerifiedProgram {
    verified_single_function_with_tables(operations, register_classes, Vec::new(), moves)
}

fn verified_single_function_with_tables(
    operations: Vec<Op>,
    register_classes: Vec<RegisterClass>,
    operands: Vec<Box<[Register]>>,
    moves: Vec<Box<[(Register, Register)]>>,
) -> crate::VerifiedProgram {
    verified_single_function_with_call_arguments(
        operations,
        register_classes,
        Vec::new(),
        operands,
        moves,
    )
}

fn verified_single_function_with_call_arguments(
    operations: Vec<Op>,
    register_classes: Vec<RegisterClass>,
    call_arguments: Vec<Box<[CallArgument]>>,
    operands: Vec<Box<[Register]>>,
    moves: Vec<Box<[(Register, Register)]>>,
) -> crate::VerifiedProgram {
    verified_single_function_with_all_tables(
        operations,
        register_classes,
        call_arguments,
        operands,
        moves,
        Vec::new(),
    )
}

fn verified_single_function_with_all_tables(
    operations: Vec<Op>,
    register_classes: Vec<RegisterClass>,
    call_arguments: Vec<Box<[CallArgument]>>,
    operands: Vec<Box<[Register]>>,
    moves: Vec<Box<[(Register, Register)]>>,
    switches: Vec<Box<[(u64, Pc)]>>,
) -> crate::VerifiedProgram {
    let function = BytecodeFunction {
        name: Name::EMPTY,
        params: Box::default(),
        return_type: VmTypeId::from_raw(Idx::INT.raw()),
        param_ownership: Box::default(),
        capture_count: 0,
        closure_adapter: None,
        ops: operations.into_boxed_slice(),
        entry: pc(0),
        register_count: register_classes.len(),
        register_types: vec![VmTypeId::from_raw(Idx::INT.raw()); register_classes.len()]
            .into_boxed_slice(),
        register_rc_strategies: vec![None; register_classes.len()].into_boxed_slice(),
        register_closure_signatures: vec![None; register_classes.len()].into_boxed_slice(),
        register_classes: register_classes.into_boxed_slice(),
    };
    let bytecode = BytecodeProgram::new(
        vec![function],
        call_arguments,
        operands,
        moves,
        switches,
        Vec::new(),
        main_function_id(),
    );
    verify(bytecode).unwrap_or_else(|error| panic!("test bytecode should verify: {error}"))
}

fn register(index: u32) -> Register {
    Register::from_arc(ArcVarId::new(index))
}

fn pc(index: usize) -> Pc {
    Pc::new(index, Name::EMPTY).unwrap_or_else(|error| panic!("test PC should fit: {error}"))
}

fn must_prepare(
    program: &crate::VerifiedProgram,
    options: PhysicalOptions,
) -> super::PhysicalVmPlan<'_> {
    prepare(program, options)
        .unwrap_or_else(|error| panic!("physical plan should prepare: {error}"))
}

fn must_prepare_candidate(
    program: &crate::VerifiedProgram,
    options: PhysicalOptions,
) -> PhysicalVmPlanCandidate<'_> {
    prepare_candidate(program, options)
        .unwrap_or_else(|error| panic!("physical candidate should prepare: {error}"))
}

fn must_execution_view<'plan>(
    plan: &'plan super::PhysicalVmPlan<'_>,
) -> super::PhysicalExecutionView<'plan> {
    plan.execution_view()
}

fn main_function_id() -> FunctionId {
    let symbols = SharedInterner::new();
    let main = symbols.intern("main");
    let function = minimal_arc_function(main);
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
    let functions = vec![function];
    let callable_facts = ori_arc::freeze_function_callable_facts(&functions, &pool);
    let executable = ExecutableProgram::validate(ExecutableProgramParts {
        version: EXECUTABLE_PROGRAM_VERSION,
        symbols,
        pool,
        functions,
        function_families: vec![FunctionFamilyTopology::new(main, Vec::new())],
        contracts: [(main, contract)].into_iter().collect(),
        function_effects,
        fresh_return_facts,
        param_disjointness,
        callable_facts,
        closure_adapters: FrozenClosureAdapters::default().adapters,
        retain_plans: RetainPlanTable::default(),
        roots: vec![main],
        cli_entry: Some(main),
        externals: Vec::new(),
        method_targets: FxHashMap::default(),
        user_drop_bindings: Vec::new(),
        repr_plan: ReprPlan::new(NarrowingPolicy::Disabled),
        type_registry: TypeRegistry::new(),
    })
    .unwrap_or_else(|error| panic!("test executable should validate: {error}"));
    executable
        .cli_entry()
        .unwrap_or_else(|| panic!("fixture must have a CLI entry"))
}

fn minimal_arc_function(name: Name) -> ArcFunction {
    ArcFunction {
        name,
        params: Vec::new(),
        return_type: Idx::INT,
        blocks: vec![ArcBlock {
            id: ArcBlockId::new(0),
            params: Vec::new(),
            body: vec![ori_arc::ArcInstr::Let {
                dst: ArcVarId::new(0),
                ty: Idx::INT,
                value: ArcValue::Literal(LitValue::Int(0)),
            }],
            terminator: ArcTerminator::Return {
                value: ArcVarId::new(0),
            },
        }],
        entry: ArcBlockId::new(0),
        var_types: vec![Idx::INT],
        var_reprs: vec![ori_arc::ValueRepr::Scalar],
        var_rc_strategies: vec![None],
        var_metadata_state: ori_arc::VariableMetadataState::Realized,
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
        class_ledger_emission: false,
    }
}
