//! Sparse fixed-point solver and shared per-PC fact replay.

use std::collections::VecDeque;

use crate::bytecode::{BytecodeFunction, BytecodeProgram, Continuation, Op, Pc};

use super::facts::{KnownFacts, PlanningScratchMetrics, ScratchAccount};
use super::flow::{block_index, discover_blocks, Block};
use super::PrepareError;

pub(super) struct KnownValueAnalysis {
    blocks: Vec<Block>,
    entries: Vec<Option<KnownFacts>>,
    scratch: ScratchAccount,
}

impl KnownValueAnalysis {
    pub(super) fn replay(
        mut self,
        function: &BytecodeFunction,
        mut visit: impl FnMut(usize, Option<&KnownFacts>) -> Result<(), PrepareError>,
    ) -> Result<PlanningScratchMetrics, PrepareError> {
        for (block_index, block) in self.blocks.iter().copied().enumerate() {
            let mut state = self.entries[block_index].take();
            for pc_index in block.start..block.end {
                visit(pc_index, state.as_ref())?;
                if let Some(state) = state.as_mut() {
                    let operation = function.ops[pc_index];
                    if !is_control(operation) {
                        apply_linear_transfer(operation, state, &mut self.scratch)?;
                    }
                }
            }
            if let Some(state) = state {
                state.release(&mut self.scratch)?;
            }
        }
        self.scratch.release_elements::<Option<KnownFacts>>(
            self.entries.capacity(),
            "sparse block-entry table",
        )?;
        self.scratch
            .release_elements::<Block>(self.blocks.capacity(), "sparse block table")?;
        self.scratch.finish()
    }
}

pub(super) fn analyze(
    program: &BytecodeProgram,
    function_index: usize,
    function: &BytecodeFunction,
) -> Result<KnownValueAnalysis, PrepareError> {
    let mut scratch = ScratchAccount::new(function_index);
    let blocks = discover_blocks(program, function, &mut scratch)?;
    let mut entries: Vec<Option<KnownFacts>> = (0..blocks.len()).map(|_| None).collect();
    scratch
        .allocate_elements::<Option<KnownFacts>>(entries.capacity(), "sparse block-entry table")?;
    let entry = block_index(&blocks, function.entry, function_index)?;
    entries[entry] = Some(KnownFacts::empty());
    let mut queued = vec![0_u8; blocks.len()];
    scratch.allocate_elements::<u8>(queued.capacity(), "sparse queued flags")?;
    let mut worklist = VecDeque::new();
    push_worklist(&mut worklist, entry, &mut scratch)?;
    queued[entry] = 1;
    solve(Solver {
        program,
        function,
        blocks: &blocks,
        entries: &mut entries,
        queued: &mut queued,
        worklist: &mut worklist,
        scratch: &mut scratch,
    })?;
    scratch.release_elements::<u8>(queued.capacity(), "sparse queued flags")?;
    scratch.release_elements::<usize>(worklist.capacity(), "sparse worklist")?;
    Ok(KnownValueAnalysis {
        blocks,
        entries,
        scratch,
    })
}

struct Solver<'a> {
    program: &'a BytecodeProgram,
    function: &'a BytecodeFunction,
    blocks: &'a [Block],
    entries: &'a mut [Option<KnownFacts>],
    queued: &'a mut [u8],
    worklist: &'a mut VecDeque<usize>,
    scratch: &'a mut ScratchAccount,
}

fn solve(mut solver: Solver<'_>) -> Result<(), PrepareError> {
    while let Some(block_index) = solver.worklist.pop_front() {
        solver.queued[block_index] = 0;
        let Some(input) = solver.entries[block_index].as_ref() else {
            continue;
        };
        let mut state = input.clone_tracked(solver.scratch)?;
        solver.process_block(block_index, &mut state)?;
        state.release(solver.scratch)?;
    }
    Ok(())
}

impl Solver<'_> {
    fn process_block(
        &mut self,
        block_index: usize,
        state: &mut KnownFacts,
    ) -> Result<(), PrepareError> {
        let block = self.blocks[block_index];
        let mut terminated = false;
        for pc_index in block.start..block.end {
            let pc = self.pc(pc_index)?;
            terminated = self.transfer(pc, self.function.ops[pc_index], state)?;
            if terminated {
                break;
            }
        }
        if !terminated && block.end < self.function.ops.len() {
            self.propagate(self.pc(block.end)?, state)?;
        }
        Ok(())
    }

    fn transfer(
        &mut self,
        pc: Pc,
        operation: Op,
        state: &mut KnownFacts,
    ) -> Result<bool, PrepareError> {
        match operation {
            Op::Call {
                dst,
                normal,
                unwind,
                ..
            }
            | Op::CallClosure {
                dst,
                normal,
                unwind,
                ..
            } => {
                let mut normal_state = state.clone_tracked(self.scratch)?;
                normal_state.forget(dst);
                self.propagate(continuation_pc(pc, normal), &normal_state)?;
                normal_state.release(self.scratch)?;
                if let Some(target) = unwind {
                    self.propagate(target, state)?;
                }
            }
            Op::Jump { target, moves } => {
                let mut output = state.clone_tracked(self.scratch)?;
                for &(destination, source) in &self.program.moves[moves.index()] {
                    output.assign_from(destination, state, source, self.scratch)?;
                }
                self.propagate(target, &output)?;
                output.release(self.scratch)?;
            }
            Op::Branch {
                then_pc, else_pc, ..
            } => {
                self.propagate(then_pc, state)?;
                self.propagate(else_pc, state)?;
            }
            Op::Switch {
                table, default_pc, ..
            } => {
                self.propagate(default_pc, state)?;
                for &(_, target) in &self.program.switches[table.index()] {
                    self.propagate(target, state)?;
                }
            }
            Op::Return { .. } | Op::Resume | Op::Unreachable => {}
            operation => {
                apply_linear_transfer(operation, state, self.scratch)?;
                return Ok(false);
            }
        }
        Ok(true)
    }

    fn propagate(&mut self, target: Pc, incoming: &KnownFacts) -> Result<(), PrepareError> {
        let target = block_index(self.blocks, target, self.scratch.function_index())?;
        let changed = match &mut self.entries[target] {
            Some(current) => current.intersect_with(incoming),
            entry @ None => {
                *entry = Some(incoming.clone_tracked(self.scratch)?);
                true
            }
        };
        if changed && self.queued[target] == 0 {
            self.queued[target] = 1;
            push_worklist(self.worklist, target, self.scratch)?;
        }
        Ok(())
    }

    fn pc(&self, index: usize) -> Result<Pc, PrepareError> {
        Pc::new(index, self.function.name).map_err(|_| PrepareError::PhysicalOpCountOverflow {
            function: self.scratch.function_index(),
            physical_ops: self.function.ops.len(),
        })
    }
}

fn apply_linear_transfer(
    operation: Op,
    state: &mut KnownFacts,
    scratch: &mut ScratchAccount,
) -> Result<(), PrepareError> {
    match operation {
        Op::Const { dst, value } => state.assign_constant(dst, value, scratch)?,
        Op::Copy { dst, src } => state.assign_copy(dst, src, scratch)?,
        Op::Binary { dst, .. }
        | Op::IntBinary { dst, .. }
        | Op::StringBinary { dst, .. }
        | Op::RuntimeBinary { dst, .. }
        | Op::Unary { dst, .. }
        | Op::BoolNot { dst, .. }
        | Op::MakeClosure { dst, .. }
        | Op::Construct { dst, .. }
        | Op::Project { dst, .. }
        | Op::IsShared { dst, .. }
        | Op::Select { dst, .. } => state.forget(dst),
        Op::SetTag { base, .. } => state.forget(base),
        Op::RcInc { .. }
        | Op::RcDec { .. }
        | Op::Set { .. }
        | Op::Call { .. }
        | Op::CallClosure { .. }
        | Op::Jump { .. }
        | Op::Branch { .. }
        | Op::Switch { .. }
        | Op::Return { .. }
        | Op::Resume
        | Op::Unreachable => {}
    }
    Ok(())
}

fn push_worklist(
    worklist: &mut VecDeque<usize>,
    block: usize,
    scratch: &mut ScratchAccount,
) -> Result<(), PrepareError> {
    let previous = worklist.capacity();
    worklist.push_back(block);
    scratch.grow_elements::<usize>(previous, worklist.capacity(), "sparse worklist")
}

const fn continuation_pc(pc: Pc, continuation: Continuation) -> Pc {
    match continuation {
        Continuation::Next => pc.next_verified(),
        Continuation::At(target) => target,
    }
}

const fn is_control(operation: Op) -> bool {
    matches!(
        operation,
        Op::Call { .. }
            | Op::CallClosure { .. }
            | Op::Jump { .. }
            | Op::Branch { .. }
            | Op::Switch { .. }
            | Op::Return { .. }
            | Op::Resume
            | Op::Unreachable
    )
}
