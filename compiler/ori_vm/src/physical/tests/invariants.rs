use ori_arc::{
    prove_param_disjointness, CtorKind, FrozenClosureAdapters, MemoryContract, RetainPlanTable,
};
use ori_ir::SharedInterner;
use ori_repr::executable::{
    ExecutableProgram, ExecutableProgramParts, FunctionFamilyTopology, FunctionId,
};
use ori_repr::{NarrowingPolicy, ReprPlan};
use ori_types::{Idx, Pool, TypeRegistry};
use rustc_hash::FxHashMap;

use crate::bytecode::{
    BytecodeFunction, BytecodeProgram, Constant, Op, OperandListId, RegisterClass, TableKind,
    VmTypeId,
};
use crate::verify;

use super::{
    minimal_arc_function, must_execution_view, must_prepare, must_prepare_candidate, pc, register,
    verified_linear_program, verified_single_function, verified_single_function_with_tables,
    PhysicalImmediate, PhysicalOptions, PhysicalRead,
};
use crate::physical::PhysicalLane;
use crate::{PhysicalLayoutViolation, PrepareError};

#[test]
fn canonical_and_physical_maps_round_trip_without_gaps_or_overlap() {
    let program = verified_linear_program();
    let plan = must_prepare(&program, PhysicalOptions::default());
    let function = must_execution_view(&plan).function_at(program.program.main.index());

    function
        .validate_construction()
        .unwrap_or_else(|error| panic!("prepared layout must satisfy release invariants: {error}"));
    let mut ownership = vec![0_usize; function.canonical_pc_count()];
    for owner in 0..function.physical_op_count() {
        let operation = function.physical_op(owner);
        let span = operation.canonical_span();
        assert_eq!(span.len(), 1);
        assert!(operation.whole_span_eligible(span.len() as u64, false));
        assert!(!operation.whole_span_eligible(0, false));
        assert!(!operation.whole_span_eligible(u64::MAX, true));
        let start = span.start().index();
        for (offset, owners) in ownership[start..start + span.len()].iter_mut().enumerate() {
            let pc_index = start + offset;
            *owners += 1;
            let reverse = function.canonical_pc_at(pc_index);
            assert_eq!(reverse.owner(), owner);
            assert_eq!(reverse.owner_offset(), offset);
            assert!(reverse
                .reads()
                .iter()
                .all(|read| matches!(read, PhysicalRead::Immediate(_))
                    || matches!(read, PhysicalRead::Lane(lane) if lane.index() < function.lane_count())));
            assert!(reverse
                .writes()
                .iter()
                .all(|lane| lane.index() < function.lane_count()));
        }
    }
    assert!(ownership.into_iter().all(|owners| owners == 1));
    assert!((0..function.canonical_pc_count())
        .all(|pc_index| function.canonical_pc_at(pc_index).owner_offset() == 0));
}

#[test]
fn side_table_operands_flatten_in_semantic_order_without_deduplication() {
    let args = OperandListId::new(0, TableKind::Operands)
        .unwrap_or_else(|error| panic!("test operand table should fit: {error}"));
    let program = verified_single_function_with_tables(
        vec![
            Op::Const {
                dst: register(0),
                value: Constant::Int(7),
            },
            Op::Const {
                dst: register(1),
                value: Constant::Bool(true),
            },
            Op::Construct {
                dst: register(2),
                ctor: CtorKind::Tuple,
                args,
            },
            Op::Return { value: register(2) },
        ],
        vec![
            RegisterClass::Int,
            RegisterClass::Bool,
            RegisterClass::Other,
        ],
        vec![vec![register(1), register(0), register(1)].into_boxed_slice()],
        Vec::new(),
    );
    let plan = must_prepare(&program, PhysicalOptions::default());
    let function = must_execution_view(&plan).function_at(program.program.main.index());

    assert_eq!(
        function.canonical_pc_at(2).reads(),
        [
            PhysicalRead::Immediate(PhysicalImmediate::Bool(true)),
            PhysicalRead::Immediate(PhysicalImmediate::Int(7)),
            PhysicalRead::Immediate(PhysicalImmediate::Bool(true)),
        ]
    );
    assert_eq!(function.canonical_pc_at(2).writes(), [function.lane_at(2)]);
    assert_eq!(plan.metrics().read_bindings, 4);
    assert_eq!(plan.metrics().write_bindings, 3);
    assert_eq!(plan.metrics().immediate_bindings, 3);
}

#[test]
fn entry_parameters_remain_dynamic_without_a_value_kind_proof() {
    let (program, parameter_function) = verified_program_with_parameter_function();
    let plan = must_prepare(&program, PhysicalOptions::default());
    let function = must_execution_view(&plan).function_at(parameter_function.index());

    assert_eq!(
        function.canonical_pc_at(0).reads(),
        [PhysicalRead::Lane(function.lane_at(0))]
    );
}

#[test]
fn unreachable_definitions_do_not_create_immediate_facts() {
    let program = verified_single_function(
        vec![
            Op::Return { value: register(0) },
            Op::Const {
                dst: register(0),
                value: Constant::Int(7),
            },
            Op::Return { value: register(0) },
        ],
        vec![RegisterClass::Int],
        Vec::new(),
    );
    let plan = must_prepare(&program, PhysicalOptions::default());
    let function = must_execution_view(&plan).function_at(program.program.main.index());

    assert_eq!(
        function.canonical_pc_at(2).reads(),
        [PhysicalRead::Lane(function.lane_at(0))]
    );
}

#[test]
fn candidate_validation_rejects_out_of_bounds_read_lanes() {
    let program = verified_linear_program();
    let mut candidate = must_prepare_candidate(&program, PhysicalOptions::default());
    let function_index = program.program.main.index();
    let invalid_lane = PhysicalLane::new(
        u32::try_from(candidate.functions[function_index].lanes.len())
            .unwrap_or_else(|_| panic!("test lane count should fit")),
    );
    let read = candidate.functions[function_index]
        .reads
        .last_mut()
        .unwrap_or_else(|| panic!("linear test plan should have reads"));
    *read = PhysicalRead::Lane(invalid_lane);

    assert!(matches!(
        candidate.validate(),
        Err(PrepareError::InvalidLayout {
            violation: PhysicalLayoutViolation::ReadLaneBounds { .. },
            ..
        })
    ));
}

#[test]
fn candidate_validation_rejects_out_of_bounds_write_lanes() {
    let program = verified_linear_program();
    let mut candidate = must_prepare_candidate(&program, PhysicalOptions::default());
    let function_index = program.program.main.index();
    let invalid_lane = PhysicalLane::new(
        u32::try_from(candidate.functions[function_index].lanes.len())
            .unwrap_or_else(|_| panic!("test lane count should fit")),
    );
    let write = candidate.functions[function_index]
        .writes
        .last_mut()
        .unwrap_or_else(|| panic!("linear test plan should have writes"));
    *write = invalid_lane;

    assert!(matches!(
        candidate.validate(),
        Err(PrepareError::InvalidLayout {
            violation: PhysicalLayoutViolation::WriteLaneBounds { .. },
            ..
        })
    ));
}

#[test]
fn candidate_validation_rejects_missing_canonical_pc() {
    let program = verified_linear_program();
    let mut candidate = must_prepare_candidate(&program, PhysicalOptions::default());
    let function_index = program.program.main.index();
    let canonical_pcs = &candidate.functions[function_index].canonical_pcs;
    let (_, canonical_prefix) = canonical_pcs
        .split_last()
        .unwrap_or_else(|| panic!("linear test plan should have canonical PCs"));
    candidate.functions[function_index].canonical_pcs =
        canonical_prefix.to_vec().into_boxed_slice();

    assert!(matches!(
        candidate.validate(),
        Err(PrepareError::InvalidLayout {
            violation: PhysicalLayoutViolation::CanonicalOpCount { .. },
            ..
        })
    ));
}

#[test]
fn candidate_validation_rejects_missing_lane() {
    let program = verified_linear_program();
    let mut candidate = must_prepare_candidate(&program, PhysicalOptions::default());
    let function_index = program.program.main.index();
    let lanes = &candidate.functions[function_index].lanes;
    let (_, lane_prefix) = lanes
        .split_last()
        .unwrap_or_else(|| panic!("linear test plan should have lanes"));
    candidate.functions[function_index].lanes = lane_prefix.to_vec().into_boxed_slice();

    assert!(matches!(
        candidate.validate(),
        Err(PrepareError::InvalidLayout {
            violation: PhysicalLayoutViolation::LaneCount { .. },
            ..
        })
    ));
}

#[test]
fn candidate_validation_rejects_missing_function() {
    let (program, _) = verified_program_with_parameter_function();
    let mut candidate = must_prepare_candidate(&program, PhysicalOptions::default());
    let mut functions = std::mem::take(&mut candidate.functions).into_vec();
    assert!(functions.pop().is_some());
    candidate.functions = functions.into_boxed_slice();

    assert!(matches!(
        candidate.validate(),
        Err(PrepareError::FunctionCountMismatch { .. })
    ));
}

#[test]
fn candidate_validation_rejects_wrong_existing_read_lane() {
    let program = verified_linear_program();
    let mut candidate = must_prepare_candidate(&program, PhysicalOptions::default());
    let function_index = program.program.main.index();
    let last_read = candidate.functions[function_index]
        .reads
        .last_mut()
        .unwrap_or_else(|| panic!("linear test plan should have reads"));
    *last_read = PhysicalRead::Lane(PhysicalLane::new(2));

    assert!(matches!(
        candidate.validate(),
        Err(PrepareError::InvalidLayout {
            violation: PhysicalLayoutViolation::ReadBindingMismatch { .. },
            ..
        })
    ));
}

#[test]
fn candidate_validation_rejects_wrong_immediate_kind() {
    let program = verified_linear_program();
    let mut candidate = must_prepare_candidate(&program, PhysicalOptions::default());
    let function_index = program.program.main.index();
    candidate.functions[function_index].reads[0] =
        PhysicalRead::Immediate(PhysicalImmediate::Bool(true));

    assert!(matches!(
        candidate.validate(),
        Err(PrepareError::InvalidLayout {
            violation: PhysicalLayoutViolation::ImmediateProofMismatch { .. },
            ..
        })
    ));
}

#[test]
fn candidate_validation_rejects_wrong_existing_write_lane() {
    let program = verified_linear_program();
    let mut candidate = must_prepare_candidate(&program, PhysicalOptions::default());
    let function_index = program.program.main.index();
    candidate.functions[function_index].writes[0] = PhysicalLane::new(1);

    assert!(matches!(
        candidate.validate(),
        Err(PrepareError::InvalidLayout {
            violation: PhysicalLayoutViolation::WriteBindingMismatch { .. },
            ..
        })
    ));
}

#[test]
fn candidate_validation_rejects_unproven_copy_noop() {
    let program = verified_linear_program();
    let mut candidate = must_prepare_candidate(&program, PhysicalOptions::default());
    let function_index = program.program.main.index();
    candidate.functions[function_index].canonical_pcs[4].set_copy_is_noop_for_test(false);

    assert!(matches!(
        candidate.validate(),
        Err(PrepareError::InvalidLayout {
            violation: PhysicalLayoutViolation::CopyNoopProofMismatch { .. },
            ..
        })
    ));
}

fn verified_program_with_parameter_function() -> (crate::VerifiedProgram, FunctionId) {
    let fixture = parameter_function_artifact();
    let main = fixture
        .executable
        .cli_entry()
        .unwrap_or_else(|| panic!("fixture must have a CLI entry"));
    let parameter_function = fixture
        .executable
        .function_id(fixture.parameter_name)
        .unwrap_or_else(|| panic!("parameter function should have an identity"));
    let mut functions = vec![
        (main, parameter_fixture_main(fixture.main_name)),
        (
            parameter_function,
            parameter_fixture_function(fixture.parameter_name),
        ),
    ];
    functions.sort_by_key(|(id, _)| *id);
    let bytecode = BytecodeProgram::new(
        functions
            .into_iter()
            .map(|(_, function)| function)
            .collect(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
        main,
    );
    let verified = verify(bytecode)
        .unwrap_or_else(|error| panic!("multi-function bytecode should verify: {error}"));
    (verified, parameter_function)
}

struct ParameterFunctionArtifact {
    executable: ExecutableProgram,
    main_name: ori_ir::Name,
    parameter_name: ori_ir::Name,
}

fn parameter_function_artifact() -> ParameterFunctionArtifact {
    let symbols = SharedInterner::new();
    let main_name = symbols.intern("a_main");
    let parameter_name = symbols.intern("z_parameter");
    let functions = vec![
        minimal_arc_function(main_name),
        minimal_arc_function(parameter_name),
    ];
    let contracts = [main_name, parameter_name]
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
        version: ori_repr::executable::EXECUTABLE_PROGRAM_VERSION,
        symbols,
        pool,
        functions,
        function_families: vec![
            FunctionFamilyTopology::new(main_name, Vec::new()),
            FunctionFamilyTopology::new(parameter_name, Vec::new()),
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
        externals: Vec::new(),
        method_targets: FxHashMap::default(),
        user_drop_bindings: Vec::new(),
        repr_plan: ReprPlan::new(NarrowingPolicy::Disabled),
        type_registry: TypeRegistry::new(),
    })
    .unwrap_or_else(|error| panic!("test executable should validate: {error}"));

    ParameterFunctionArtifact {
        executable,
        main_name,
        parameter_name,
    }
}

fn parameter_fixture_main(name: ori_ir::Name) -> BytecodeFunction {
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
                value: Constant::Int(0),
            },
            Op::Return { value: register(0) },
        ]
        .into_boxed_slice(),
        entry: pc(0),
        register_count: 1,
        register_types: vec![VmTypeId::from_raw(Idx::INT.raw())].into_boxed_slice(),
        register_classes: vec![RegisterClass::Int].into_boxed_slice(),
        register_rc_strategies: vec![None].into_boxed_slice(),
        register_closure_signatures: vec![None].into_boxed_slice(),
    }
}

fn parameter_fixture_function(name: ori_ir::Name) -> BytecodeFunction {
    BytecodeFunction {
        name,
        params: vec![register(0)].into_boxed_slice(),
        return_type: VmTypeId::from_raw(Idx::INT.raw()),
        param_ownership: vec![ori_arc::Ownership::Borrowed].into_boxed_slice(),
        capture_count: 0,
        closure_adapter: None,
        ops: vec![Op::Return { value: register(0) }].into_boxed_slice(),
        entry: pc(0),
        register_count: 1,
        register_types: vec![VmTypeId::from_raw(Idx::INT.raw())].into_boxed_slice(),
        register_classes: vec![RegisterClass::Int].into_boxed_slice(),
        register_rc_strategies: vec![None].into_boxed_slice(),
        register_closure_signatures: vec![None].into_boxed_slice(),
    }
}
