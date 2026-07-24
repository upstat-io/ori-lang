use std::hint::black_box;
use std::time::Instant;

use crate::bytecode::{Constant, MoveListId, Op, RegisterClass, SwitchTableId, TableKind};

use super::{
    must_execution_view, must_prepare, pc, register, verified_single_function,
    verified_single_function_with_all_tables, PhysicalImmediate, PhysicalOptions, PhysicalRead,
};

const HIGH_REGISTER_COUNTS: [usize; 2] = [4_096, 16_384];

#[test]
fn high_register_cfg_preserves_exact_physical_projection() {
    let program = high_register_cfg(HIGH_REGISTER_COUNTS[0]);
    let plan = must_prepare(&program, PhysicalOptions::default());
    let function = must_execution_view(&plan).function_at(program.program.main.index());
    let high = HIGH_REGISTER_COUNTS[0] - 1;
    let unreachable = high - 1;

    assert_eq!(
        function.canonical_pc_at(4).writes(),
        [function.lane_at(high)]
    );
    assert_eq!(
        function.canonical_pc_at(4).reads(),
        [PhysicalRead::Immediate(PhysicalImmediate::Int(7))]
    );
    assert_eq!(
        function.canonical_pc_at(6).writes(),
        [function.lane_at(high)]
    );
    assert_eq!(
        function.canonical_pc_at(6).reads(),
        [PhysicalRead::Immediate(PhysicalImmediate::Int(9))]
    );
    assert_eq!(
        function.canonical_pc_at(7).reads(),
        [PhysicalRead::Lane(function.lane_at(high))]
    );
    assert_eq!(
        function.canonical_pc_at(10).reads(),
        [
            PhysicalRead::Immediate(PhysicalImmediate::Int(2)),
            PhysicalRead::Immediate(PhysicalImmediate::Int(1)),
        ]
    );
    assert_eq!(
        function.canonical_pc_at(10).writes(),
        [function.lane_at(4), function.lane_at(5)]
    );
    assert_eq!(
        function.canonical_pc_at(11).reads(),
        [PhysicalRead::Lane(function.lane_at(4))]
    );
    assert!(function.canonical_pc_at(12).copy_is_noop());
    assert_eq!(
        function.canonical_pc_at(17).reads(),
        [PhysicalRead::Lane(function.lane_at(unreachable))]
    );
    assert_eq!(plan.metrics().planning_scratch_current_payload_bytes, 0);
    assert_eq!(plan.metrics().validation_scratch_current_payload_bytes, 0);
}

#[test]
fn high_register_cfg_reports_planning_time_and_separate_scratch() {
    for register_count in HIGH_REGISTER_COUNTS {
        let program = high_register_cfg(register_count);
        black_box(must_prepare(&program, PhysicalOptions::default()));
        let started = Instant::now();
        let mut latest = None;
        for _ in 0..20 {
            latest = Some(black_box(must_prepare(
                &program,
                PhysicalOptions::default(),
            )));
        }
        let elapsed = started.elapsed();
        let metrics = latest
            .as_ref()
            .unwrap_or_else(|| panic!("measurement loop must prepare a plan"))
            .metrics();
        assert_eq!(metrics.planning_scratch_current_payload_bytes, 0);
        assert_eq!(metrics.validation_scratch_current_payload_bytes, 0);
        eprintln!(
            "physical_planning_adversarial registers={} iterations=20 elapsed_ns={} retained_plan_bytes={} planning_scratch_peak_payload_bytes_lower_bound={} planning_scratch_cumulative_allocation_bytes_lower_bound={} validation_scratch_peak_payload_bytes_lower_bound={} validation_scratch_cumulative_allocation_bytes_lower_bound={}",
            register_count,
            elapsed.as_nanos(),
            metrics.owned_plan_bytes,
            metrics.planning_scratch_peak_payload_bytes_lower_bound,
            metrics.planning_scratch_cumulative_allocation_bytes_lower_bound,
            metrics.validation_scratch_peak_payload_bytes_lower_bound,
            metrics.validation_scratch_cumulative_allocation_bytes_lower_bound,
        );
    }
}

#[test]
fn switch_join_intersects_exact_facts_without_edge_state_copies() {
    let switch = SwitchTableId::new(0, TableKind::Switches)
        .unwrap_or_else(|error| panic!("test switch table should fit: {error}"));
    let program = verified_single_function_with_all_tables(
        vec![
            Op::Const {
                dst: register(0),
                value: Constant::Int(1),
            },
            Op::Switch {
                scrutinee: register(0),
                table: switch,
                default_pc: pc(4),
            },
            Op::Const {
                dst: register(1),
                value: Constant::Int(7),
            },
            Op::Jump {
                target: pc(6),
                moves: move_id(0),
            },
            Op::Const {
                dst: register(1),
                value: Constant::Int(9),
            },
            Op::Jump {
                target: pc(6),
                moves: move_id(0),
            },
            Op::Return { value: register(1) },
        ],
        vec![RegisterClass::Int, RegisterClass::Int],
        Vec::new(),
        Vec::new(),
        vec![Box::default()],
        vec![vec![(1, pc(2))].into_boxed_slice()],
    );
    let plan = must_prepare(&program, PhysicalOptions::default());
    let function = must_execution_view(&plan).function_at(program.program.main.index());

    assert_eq!(
        function.canonical_pc_at(1).reads(),
        [PhysicalRead::Immediate(PhysicalImmediate::Int(1))]
    );
    assert_eq!(
        function.canonical_pc_at(6).reads(),
        [PhysicalRead::Lane(function.lane_at(1))]
    );
}

fn high_register_cfg(register_count: usize) -> crate::VerifiedProgram {
    let join_moves = move_id(0);
    let parallel_moves = move_id(1);
    let high = u32::try_from(register_count - 1)
        .unwrap_or_else(|_| panic!("test register count must fit"));
    let unreachable = high - 1;
    let operations = vec![
        Op::Const {
            dst: register(0),
            value: Constant::Bool(true),
        },
        Op::Const {
            dst: register(1),
            value: Constant::Int(7),
        },
        Op::Branch {
            cond: register(0),
            then_pc: pc(3),
            else_pc: pc(5),
        },
        Op::Copy {
            dst: register(2),
            src: register(1),
        },
        Op::Jump {
            target: pc(7),
            moves: join_moves,
        },
        Op::Const {
            dst: register(2),
            value: Constant::Int(9),
        },
        Op::Jump {
            target: pc(7),
            moves: join_moves,
        },
        Op::Copy {
            dst: register(3),
            src: register(high),
        },
        Op::Const {
            dst: register(4),
            value: Constant::Int(1),
        },
        Op::Const {
            dst: register(5),
            value: Constant::Int(2),
        },
        Op::Jump {
            target: pc(11),
            moves: parallel_moves,
        },
        Op::Copy {
            dst: register(6),
            src: register(4),
        },
        Op::Copy {
            dst: register(3),
            src: register(3),
        },
        Op::Const {
            dst: register(4),
            value: Constant::Int(3),
        },
        Op::Branch {
            cond: register(0),
            then_pc: pc(11),
            else_pc: pc(15),
        },
        Op::Return { value: register(3) },
        Op::Const {
            dst: register(unreachable),
            value: Constant::Int(55),
        },
        Op::Return {
            value: register(unreachable),
        },
    ];
    let mut classes = vec![RegisterClass::Other; register_count];
    classes[0] = RegisterClass::Bool;
    for class in &mut classes[1..7] {
        *class = RegisterClass::Int;
    }
    verified_single_function(
        operations,
        classes,
        vec![
            vec![(register(high), register(2))].into_boxed_slice(),
            vec![(register(4), register(5)), (register(5), register(4))].into_boxed_slice(),
        ],
    )
}

fn move_id(index: usize) -> MoveListId {
    MoveListId::new(index, TableKind::Moves)
        .unwrap_or_else(|error| panic!("test move table should fit: {error}"))
}
