//! Flattened physical operands constructed from sparse exact facts.

use crate::bytecode::{
    walk_register_operands, BytecodeFunction, BytecodeProgram, Op, Pc, Register, RegisterOperand,
};

use super::facts::{KnownFacts, KnownValue, PlanningScratchMetrics};
use super::layout::{
    OperandSpan, PhysicalFunctionPlan, PhysicalOpIndex, PhysicalOpPlan, PhysicalPcPlan,
};
use super::solver::analyze;
use super::{PhysicalImmediate, PhysicalLane, PhysicalOptions, PhysicalRead, PrepareError};

pub(super) fn prepare_function(
    program: &BytecodeProgram,
    function_index: usize,
    function: &BytecodeFunction,
    options: PhysicalOptions,
) -> Result<(PhysicalFunctionPlan, PlanningScratchMetrics), PrepareError> {
    let lane_count = function.register_count;
    let lanes = (0..lane_count)
        .map(|lane| {
            u32::try_from(lane).map(PhysicalLane::new).map_err(|_| {
                PrepareError::LaneCountOverflow {
                    function: function_index,
                    logical_lanes: lane_count,
                }
            })
        })
        .collect::<Result<Vec<_>, _>>()?
        .into_boxed_slice();
    let analysis = analyze(program, function_index, function)?;
    let mut physical_ops = Vec::with_capacity(function.ops.len());
    let mut canonical_pcs = Vec::with_capacity(function.ops.len());
    let mut reads = Vec::new();
    let mut writes = Vec::new();
    let planning = analysis.replay(function, |pc_index, state| {
        let owner = PhysicalOpIndex::new(physical_ops.len()).ok_or(
            PrepareError::PhysicalOpCountOverflow {
                function: function_index,
                physical_ops: function.ops.len(),
            },
        )?;
        let pc = Pc::new(pc_index, function.name).map_err(|_| {
            PrepareError::PhysicalOpCountOverflow {
                function: function_index,
                physical_ops: function.ops.len(),
            }
        })?;
        let operation = function.ops[pc_index];
        let read_start = reads.len();
        let write_start = writes.len();
        walk_register_operands(program, operation, |operand| match operand {
            RegisterOperand::Read(register) => reads.push(physical_read(
                &lanes,
                state,
                register,
                options.bind_immediates,
            )),
            RegisterOperand::Write(register) => writes.push(lanes[register.index()]),
        })
        .map_err(|error| PrepareError::CanonicalOperandTable {
            function: function_index,
            canonical_pc: pc_index,
            table: error.table,
            index: error.index,
            bound: error.bound,
        })?;
        let read_span = OperandSpan::new(read_start, reads.len() - read_start).ok_or(
            PrepareError::ReadBindingCountOverflow {
                function: function_index,
                read_bindings: reads.len(),
            },
        )?;
        let write_span = OperandSpan::new(write_start, writes.len() - write_start).ok_or(
            PrepareError::WriteBindingCountOverflow {
                function: function_index,
                write_bindings: writes.len(),
            },
        )?;
        let copy_is_noop =
            options.coalesce_copies && matches!(operation, Op::Copy { dst, src } if dst == src);
        physical_ops.push(PhysicalOpPlan::identity(pc));
        canonical_pcs.push(PhysicalPcPlan::new(
            owner,
            0,
            read_span,
            write_span,
            copy_is_noop,
        ));
        Ok(())
    })?;
    u32::try_from(reads.len()).map_err(|_| PrepareError::ReadBindingCountOverflow {
        function: function_index,
        read_bindings: reads.len(),
    })?;
    u32::try_from(writes.len()).map_err(|_| PrepareError::WriteBindingCountOverflow {
        function: function_index,
        write_bindings: writes.len(),
    })?;
    Ok((
        PhysicalFunctionPlan {
            lanes,
            physical_ops: physical_ops.into_boxed_slice(),
            canonical_pcs: canonical_pcs.into_boxed_slice(),
            reads: reads.into_boxed_slice(),
            writes: writes.into_boxed_slice(),
        },
        planning,
    ))
}

pub(super) fn physical_read(
    lanes: &[PhysicalLane],
    state: Option<&KnownFacts>,
    register: Register,
    bind_immediates: bool,
) -> PhysicalRead {
    let immediate = if bind_immediates {
        state
            .and_then(|state| state.get(register))
            .map(immediate_value)
    } else {
        None
    };
    immediate.map_or(
        PhysicalRead::Lane(lanes[register.index()]),
        PhysicalRead::Immediate,
    )
}

const fn immediate_value(value: KnownValue) -> PhysicalImmediate {
    match value {
        KnownValue::Int(value) => PhysicalImmediate::Int(value),
        KnownValue::Bool(value) => PhysicalImmediate::Bool(value),
    }
}
