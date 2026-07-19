//! Canonical control-flow boundaries for sparse physical planning.

use crate::bytecode::{BytecodeFunction, BytecodeProgram, Continuation, Op, Pc};

use super::facts::ScratchAccount;
use super::PrepareError;

#[derive(Clone, Copy)]
pub(super) struct Block {
    pub(super) start: usize,
    pub(super) end: usize,
}

pub(super) fn discover_blocks(
    program: &BytecodeProgram,
    function: &BytecodeFunction,
    scratch: &mut ScratchAccount,
) -> Result<Vec<Block>, PrepareError> {
    let mut starts = vec![0_u8; function.ops.len()];
    scratch.allocate_elements::<u8>(starts.capacity(), "sparse block markers")?;
    starts[0] = 1;
    starts[function.entry.index()] = 1;
    for (pc_index, &operation) in function.ops.iter().enumerate() {
        match operation {
            Op::Call { normal, unwind, .. } => {
                starts[continuation_pc_index(pc_index, normal)] = 1;
                if let Some(target) = unwind {
                    starts[target.index()] = 1;
                }
            }
            Op::Jump { target, .. } => starts[target.index()] = 1,
            Op::Branch {
                then_pc, else_pc, ..
            } => {
                starts[then_pc.index()] = 1;
                starts[else_pc.index()] = 1;
            }
            Op::Switch {
                table, default_pc, ..
            } => {
                starts[default_pc.index()] = 1;
                for &(_, target) in &program.switches[table.index()] {
                    starts[target.index()] = 1;
                }
            }
            Op::Return { .. } | Op::Resume | Op::Unreachable => {}
            _ => continue,
        }
        if pc_index + 1 < starts.len() {
            starts[pc_index + 1] = 1;
        }
    }
    let block_count = starts.iter().filter(|&&start| start != 0).count();
    let mut blocks = Vec::with_capacity(block_count);
    scratch.allocate_elements::<Block>(blocks.capacity(), "sparse block table")?;
    let mut previous = None;
    for (start, &marked) in starts.iter().enumerate() {
        if marked == 0 {
            continue;
        }
        if let Some(previous) = previous.replace(start) {
            blocks.push(Block {
                start: previous,
                end: start,
            });
        }
    }
    if let Some(start) = previous {
        blocks.push(Block {
            start,
            end: function.ops.len(),
        });
    }
    scratch.release_elements::<u8>(starts.capacity(), "sparse block markers")?;
    Ok(blocks)
}

pub(super) fn block_index(
    blocks: &[Block],
    target: Pc,
    function: usize,
) -> Result<usize, PrepareError> {
    blocks
        .binary_search_by_key(&target.index(), |block| block.start)
        .map_err(|_| PrepareError::PlanningBlockTargetMissing {
            function,
            target: target.index(),
        })
}

const fn continuation_pc_index(pc: usize, continuation: Continuation) -> usize {
    match continuation {
        Continuation::Next => pc + 1,
        Continuation::At(target) => target.index(),
    }
}
