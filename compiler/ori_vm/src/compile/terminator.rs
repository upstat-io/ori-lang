//! ARC terminator lowering.

use ori_arc::{ArcFunction, ArcTerminator};
use ori_repr::executable::{BlockIndex, CallPosition, CallSite};

use super::{block_pc, Compiler};
use crate::bytecode::{Continuation, Op, Pc, Register};
use crate::CompileError;

impl Compiler<'_> {
    pub(super) fn compile_terminator(
        &mut self,
        function: &ArcFunction,
        block_index: usize,
        starts: &[Pc],
        terminator: &ArcTerminator,
    ) -> Result<Op, CompileError> {
        match terminator {
            ArcTerminator::Return { value } => Ok(Op::Return {
                value: Register::from_arc(*value),
            }),
            ArcTerminator::Jump { target, args } => {
                let target_block =
                    function
                        .blocks
                        .get(target.index())
                        .ok_or(CompileError::InvalidBlock {
                            function: function.name,
                            block: target.index(),
                        })?;
                if target_block.params.len() != args.len() {
                    return Err(CompileError::JumpArity {
                        function: function.name,
                        expected: target_block.params.len(),
                        actual: args.len(),
                    });
                }
                let moves = target_block
                    .params
                    .iter()
                    .zip(args)
                    .map(|((destination, _), source)| {
                        (
                            Register::from_arc(*destination),
                            Register::from_arc(*source),
                        )
                    })
                    .collect();
                Ok(Op::Jump {
                    target: block_pc(function.name, starts, *target)?,
                    moves: self.add_moves(moves)?,
                })
            }
            ArcTerminator::Branch {
                cond,
                then_block,
                else_block,
            } => Ok(Op::Branch {
                cond: Register::from_arc(*cond),
                then_pc: block_pc(function.name, starts, *then_block)?,
                else_pc: block_pc(function.name, starts, *else_block)?,
            }),
            ArcTerminator::Switch {
                scrutinee,
                cases,
                default,
            } => {
                let cases = cases
                    .iter()
                    .map(|(value, block)| {
                        block_pc(function.name, starts, *block).map(|pc| (*value, pc))
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(Op::Switch {
                    scrutinee: Register::from_arc(*scrutinee),
                    table: self.add_switch(cases)?,
                    default_pc: block_pc(function.name, starts, *default)?,
                })
            }
            ArcTerminator::Invoke {
                dst,
                args,
                arg_ownership,
                normal,
                unwind,
                ..
            } => {
                let callee = super::validate_vm_call_target(
                    function.name,
                    self.terminator_target(function, block_index)?,
                )?;
                Ok(Op::Call {
                    dst: Register::from_arc(*dst),
                    callee,
                    args: self.add_call_arguments(function.name, args, arg_ownership)?,
                    normal: Continuation::At(block_pc(function.name, starts, *normal)?),
                    unwind: Some(block_pc(function.name, starts, *unwind)?),
                })
            }
            ArcTerminator::InvokeIndirect {
                dst,
                closure,
                args,
                arg_ownership,
                normal,
                unwind,
                ..
            } => Ok(Op::CallClosure {
                dst: Register::from_arc(*dst),
                closure: Register::from_arc(*closure),
                args: self.add_call_arguments(function.name, args, arg_ownership)?,
                normal: Continuation::At(block_pc(function.name, starts, *normal)?),
                unwind: Some(block_pc(function.name, starts, *unwind)?),
            }),
            ArcTerminator::Resume => Ok(Op::Resume),
            ArcTerminator::Unreachable => Ok(Op::Unreachable),
        }
    }

    fn terminator_target(
        &self,
        function: &ArcFunction,
        block_index: usize,
    ) -> Result<ori_repr::executable::CallableTarget, CompileError> {
        let function_id =
            self.source
                .function_id(function.name)
                .ok_or(CompileError::MissingCallTarget {
                    function: function.name,
                    block: block_index,
                    position: function.blocks[block_index].body.len(),
                })?;
        let block = BlockIndex::new(block_index, function.name).map_err(|_| {
            CompileError::FunctionTooLarge {
                function: function.name,
                count: block_index,
            }
        })?;
        self.source
            .call_target(CallSite::new(function_id, block, CallPosition::Terminator))
            .ok_or(CompileError::MissingCallTarget {
                function: function.name,
                block: block_index,
                position: function.blocks[block_index].body.len(),
            })
    }
}
