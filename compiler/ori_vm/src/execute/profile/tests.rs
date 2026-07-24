use ori_arc::{
    ArcBlock, ArcBlockId, ArcFunction, ArcInstr, ArcParam, ArcTerminator, ArcValue, ArcVarId,
    ArgOwnership, LitValue, Ownership, PrimOp,
};
use ori_ir::{BinaryOp, SharedInterner};
use ori_types::{Idx, Pool};

use super::{OpcodeCount, OpcodePairCount, ProfileFunctionId, ProfilePc, RegionCount};
use crate::execute::tests::{verified_list_program, verified_program};
use crate::{execute_profiled, ExecutionConfig, ExecutionError, ExitValue, OpcodeKind};

#[test]
fn dispatch_profile_success_records_exact_opcode_pairs_and_region() {
    let program = verified_list_program(
        true,
        ArcTerminator::Return {
            value: ArcVarId::new(0),
        },
    );

    let report = execute_profiled(&program, ExecutionConfig::default());

    assert!(std::ptr::eq(
        std::ptr::from_ref(report.profile.program()),
        std::ptr::from_ref(&program),
    ));
    assert!(matches!(report.execution.result, Ok(ExitValue::Int(7))));
    assert_eq!(
        report.profile.opcodes,
        vec![
            opcode(OpcodeKind::Const, 1),
            opcode(OpcodeKind::Construct, 1),
            opcode(OpcodeKind::RcDec, 1),
            opcode(OpcodeKind::Return, 1),
        ],
    );
    assert_eq!(
        report.profile.all_pairs,
        vec![
            pair(OpcodeKind::Const, OpcodeKind::Construct, 1),
            pair(OpcodeKind::Construct, OpcodeKind::RcDec, 1),
            pair(OpcodeKind::RcDec, OpcodeKind::Return, 1),
        ],
    );
    assert_eq!(
        report.profile.linear_fallthrough_pairs,
        report.profile.all_pairs,
    );
    assert_eq!(report.profile.regions, vec![region(0, 0, 4, 1, 4)],);
    assert_profile_conservation(&report);
}

#[test]
fn dispatch_profile_step_quota_records_only_dispatched_prefix() {
    let program = verified_list_program(
        false,
        ArcTerminator::Return {
            value: ArcVarId::new(0),
        },
    );
    let config = ExecutionConfig {
        max_steps: 2,
        ..ExecutionConfig::default()
    };

    let report = execute_profiled(&program, config);

    assert!(matches!(
        report.execution.result,
        Err(ExecutionError::StepLimit { limit: 2 })
    ));
    assert_eq!(
        report.profile.opcodes,
        vec![
            opcode(OpcodeKind::Const, 1),
            opcode(OpcodeKind::Construct, 1),
        ],
    );
    assert_eq!(
        report.profile.all_pairs,
        vec![pair(OpcodeKind::Const, OpcodeKind::Construct, 1)],
    );
    assert_eq!(
        report.profile.linear_fallthrough_pairs,
        report.profile.all_pairs,
    );
    assert_eq!(report.profile.regions, vec![region(0, 0, 3, 1, 2)],);
    assert_eq!(report.execution.metrics.final_live_heap_objects, 0);
    assert_eq!(report.execution.metrics.final_live_heap_payload_bytes, 0);
    assert_profile_conservation(&report);
}

#[test]
fn dispatch_profile_records_cross_frame_transitions_without_fusing_them() {
    let symbols = SharedInterner::new();
    let callee = symbols.intern("profile_callee");
    let main = symbols.intern("profile_main");
    let pool = Pool::new();
    let callee_function = test_function(
        callee,
        vec![ArcParam {
            var: var(0),
            ty: Idx::INT,
            ownership: Ownership::Borrowed,
        }],
        vec![block(0, vec![], ArcTerminator::Return { value: var(0) })],
        vec![Idx::INT],
    );
    let main_function = test_function(
        main,
        vec![],
        vec![
            block(
                0,
                vec![int_literal(0, 7)],
                ArcTerminator::Invoke {
                    dst: var(1),
                    ty: Idx::INT,
                    func: callee,
                    args: vec![var(0)],
                    arg_ownership: vec![ArgOwnership::Borrowed],
                    mono_instance_id: None,
                    normal: block_id(1),
                    unwind: block_id(2),
                },
            ),
            block(1, vec![], ArcTerminator::Return { value: var(1) }),
            block(2, vec![], ArcTerminator::Resume),
        ],
        vec![Idx::INT, Idx::INT],
    );
    let program = verified_program(symbols, pool, vec![main_function, callee_function], main);

    let report = execute_profiled(&program, ExecutionConfig::default());

    assert!(matches!(report.execution.result, Ok(ExitValue::Int(7))));
    assert_eq!(
        report.profile.opcodes,
        vec![
            opcode(OpcodeKind::Const, 1),
            opcode(OpcodeKind::Call, 1),
            opcode(OpcodeKind::Return, 2),
        ],
    );
    assert_eq!(
        report.profile.all_pairs,
        vec![
            pair(OpcodeKind::Const, OpcodeKind::Call, 1),
            pair(OpcodeKind::Call, OpcodeKind::Return, 1),
            pair(OpcodeKind::Return, OpcodeKind::Return, 1),
        ],
    );
    assert_eq!(
        report.profile.linear_fallthrough_pairs,
        vec![pair(OpcodeKind::Const, OpcodeKind::Call, 1)],
    );
    assert_eq!(
        report.profile.regions,
        vec![
            region(0, 0, 1, 1, 1),
            region(1, 0, 2, 1, 2),
            region(1, 2, 3, 1, 1),
        ],
    );
    assert_profile_conservation(&report);
}

#[test]
fn dispatch_profile_records_exact_loop_cfg_vectors_and_regions() {
    let symbols = SharedInterner::new();
    let main = symbols.intern("profile_loop_main");
    let pool = Pool::new();
    let function = test_function(
        main,
        vec![],
        vec![
            block(
                0,
                vec![int_literal(0, 2)],
                ArcTerminator::Jump {
                    target: block_id(1),
                    args: vec![var(0)],
                },
            ),
            ArcBlock {
                id: block_id(1),
                params: vec![(var(1), Idx::INT)],
                body: vec![int_literal(2, 0), binary(3, Idx::BOOL, BinaryOp::Eq, 1, 2)],
                terminator: ArcTerminator::Branch {
                    cond: var(3),
                    then_block: block_id(3),
                    else_block: block_id(2),
                },
            },
            block(
                2,
                vec![int_literal(4, 1), binary(5, Idx::INT, BinaryOp::Sub, 1, 4)],
                ArcTerminator::Jump {
                    target: block_id(1),
                    args: vec![var(5)],
                },
            ),
            block(3, vec![], ArcTerminator::Return { value: var(1) }),
        ],
        vec![Idx::INT, Idx::INT, Idx::INT, Idx::BOOL, Idx::INT, Idx::INT],
    );
    let program = verified_program(symbols, pool, vec![function], main);

    let report = execute_profiled(&program, ExecutionConfig::default());

    assert!(matches!(report.execution.result, Ok(ExitValue::Int(0))));
    assert_eq!(
        report.profile.opcodes,
        vec![
            opcode(OpcodeKind::Const, 6),
            opcode(OpcodeKind::IntBinary, 5),
            opcode(OpcodeKind::Jump, 3),
            opcode(OpcodeKind::Branch, 3),
            opcode(OpcodeKind::Return, 1),
        ],
    );
    assert_eq!(
        report.profile.all_pairs,
        vec![
            pair(OpcodeKind::Const, OpcodeKind::IntBinary, 5),
            pair(OpcodeKind::Const, OpcodeKind::Jump, 1),
            pair(OpcodeKind::IntBinary, OpcodeKind::Jump, 2),
            pair(OpcodeKind::IntBinary, OpcodeKind::Branch, 3),
            pair(OpcodeKind::Jump, OpcodeKind::Const, 3),
            pair(OpcodeKind::Branch, OpcodeKind::Const, 2),
            pair(OpcodeKind::Branch, OpcodeKind::Return, 1),
        ],
    );
    assert_eq!(
        report.profile.linear_fallthrough_pairs,
        vec![
            pair(OpcodeKind::Const, OpcodeKind::IntBinary, 5),
            pair(OpcodeKind::Const, OpcodeKind::Jump, 1),
            pair(OpcodeKind::IntBinary, OpcodeKind::Jump, 2),
            pair(OpcodeKind::IntBinary, OpcodeKind::Branch, 3),
        ],
    );
    assert_eq!(
        report.profile.regions,
        vec![
            region(0, 0, 2, 1, 2),
            region(0, 2, 5, 3, 9),
            region(0, 5, 8, 2, 6),
            region(0, 8, 9, 1, 1),
        ],
    );
    assert_profile_conservation(&report);
}

#[test]
fn dispatch_profile_invoke_panic_records_exact_unwind_prefix() {
    let symbols = SharedInterner::new();
    let main = symbols.intern("profile_panic_main");
    let panic = symbols.intern("ori_panic");
    let message = symbols.intern("profile panic");
    let pool = Pool::new();
    let function = test_function(
        main,
        vec![],
        vec![
            block(
                0,
                vec![ArcInstr::Let {
                    dst: var(0),
                    ty: Idx::STR,
                    value: ArcValue::Literal(LitValue::String(message)),
                }],
                ArcTerminator::Invoke {
                    dst: var(1),
                    ty: Idx::UNIT,
                    func: panic,
                    args: vec![var(0)],
                    arg_ownership: vec![ArgOwnership::Borrowed],
                    mono_instance_id: None,
                    normal: block_id(1),
                    unwind: block_id(2),
                },
            ),
            block(
                1,
                vec![int_literal(2, 99)],
                ArcTerminator::Return { value: var(2) },
            ),
            block(2, vec![], ArcTerminator::Resume),
        ],
        vec![Idx::STR, Idx::UNIT, Idx::INT],
    );
    let program = verified_program(symbols, pool, vec![function], main);

    let report = execute_profiled(&program, ExecutionConfig::default());

    assert!(matches!(
        &report.execution.result,
        Err(ExecutionError::Panic { message }) if message == "profile panic"
    ));
    assert_eq!(
        report.profile.opcodes,
        vec![
            opcode(OpcodeKind::Const, 1),
            opcode(OpcodeKind::Call, 1),
            opcode(OpcodeKind::Resume, 1),
        ],
    );
    assert_eq!(
        report.profile.all_pairs,
        vec![
            pair(OpcodeKind::Const, OpcodeKind::Call, 1),
            pair(OpcodeKind::Call, OpcodeKind::Resume, 1),
        ],
    );
    assert_eq!(
        report.profile.linear_fallthrough_pairs,
        vec![pair(OpcodeKind::Const, OpcodeKind::Call, 1)],
    );
    assert_eq!(
        report.profile.regions,
        vec![region(0, 0, 2, 1, 2), region(0, 4, 5, 1, 1)],
    );
    assert_eq!(report.execution.metrics.final_live_heap_objects, 0);
    assert_eq!(report.execution.metrics.final_live_heap_payload_bytes, 0);
    assert_profile_conservation(&report);
}

fn assert_profile_conservation(report: &super::ProfiledExecutionReport<'_>) {
    let profile = &report.profile;
    assert_eq!(profile.dispatches, report.execution.metrics.steps);
    assert_eq!(
        profile
            .opcodes
            .iter()
            .map(|row| row.dispatches)
            .sum::<u64>(),
        profile.dispatches,
    );
    assert_eq!(
        profile
            .all_pairs
            .iter()
            .map(|row| row.dispatches)
            .sum::<u64>(),
        profile.dispatches.saturating_sub(1),
    );
    assert_eq!(
        profile
            .regions
            .iter()
            .map(|row| row.dispatches)
            .sum::<u64>(),
        profile.dispatches,
    );
    for linear in &profile.linear_fallthrough_pairs {
        let all_dispatches = profile
            .all_pairs
            .iter()
            .find(|all| all.first == linear.first && all.second == linear.second)
            .map_or(0, |all| all.dispatches);
        assert!(linear.dispatches <= all_dispatches);
    }
}

const fn opcode(opcode: OpcodeKind, dispatches: u64) -> OpcodeCount {
    OpcodeCount { opcode, dispatches }
}

const fn pair(first: OpcodeKind, second: OpcodeKind, dispatches: u64) -> OpcodePairCount {
    OpcodePairCount {
        first,
        second,
        dispatches,
    }
}

const fn region(
    function: usize,
    start: usize,
    end: usize,
    entries: u64,
    dispatches: u64,
) -> RegionCount {
    RegionCount {
        function: ProfileFunctionId(function),
        start: ProfilePc(start),
        end: ProfilePc(end),
        entries,
        dispatches,
    }
}

fn test_function(
    name: ori_ir::Name,
    params: Vec<ArcParam>,
    blocks: Vec<ArcBlock>,
    var_types: Vec<Idx>,
) -> ArcFunction {
    let spans = blocks
        .iter()
        .map(|block| vec![None; block.body.len()])
        .collect();
    ArcFunction {
        name,
        params,
        return_type: Idx::INT,
        blocks,
        entry: block_id(0),
        var_types,
        spans,
        ..ArcFunction::default()
    }
}

fn block(id: u32, body: Vec<ArcInstr>, terminator: ArcTerminator) -> ArcBlock {
    ArcBlock {
        id: block_id(id),
        params: vec![],
        body,
        terminator,
    }
}

fn int_literal(destination: u32, value: i64) -> ArcInstr {
    ArcInstr::Let {
        dst: var(destination),
        ty: Idx::INT,
        value: ArcValue::Literal(LitValue::Int(value)),
    }
}

fn binary(destination: u32, ty: Idx, operation: BinaryOp, left: u32, right: u32) -> ArcInstr {
    ArcInstr::Let {
        dst: var(destination),
        ty,
        value: ArcValue::PrimOp {
            op: PrimOp::Binary(operation),
            args: vec![var(left), var(right)],
        },
    }
}

fn block_id(index: u32) -> ArcBlockId {
    ArcBlockId::new(index)
}

fn var(index: u32) -> ArcVarId {
    ArcVarId::new(index)
}
