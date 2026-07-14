//! ARC instruction lowering.

use ori_arc::{ArcFunction, ArcInstr, ArcValue, LitValue, PrimOp};
use ori_repr::{
    binary_primitive_strategy,
    executable::{BlockIndex, CallPosition, CallSite},
    unary_primitive_strategy, BuiltinType, PrimitiveStrategy,
};

use super::Compiler;
use crate::bytecode::{Constant, Continuation, IntBinaryOp, Op, Register, StringBinaryOp};
use crate::{ArcInstructionKind, CompileError};

const MAX_LOCAL_AGGREGATE_FIELDS: usize = 4;

impl Compiler<'_> {
    pub(super) fn compile_instruction(
        &mut self,
        function: &ArcFunction,
        block_index: usize,
        instruction_index: usize,
        instruction: &ArcInstr,
    ) -> Result<Op, CompileError> {
        match instruction {
            ArcInstr::Let { dst, value, .. } => self.compile_let(function, *dst, value),
            ArcInstr::Apply { dst, args, .. } => {
                let callee = self.call_target(function, block_index, instruction_index)?;
                Ok(Op::Call {
                    dst: Register::from_arc(*dst),
                    callee,
                    args: self.add_operands(args)?,
                    normal: Continuation::Next,
                    unwind: None,
                })
            }
            ArcInstr::Project {
                dst, value, field, ..
            } => Ok(Op::Project {
                dst: Register::from_arc(*dst),
                value: Register::from_arc(*value),
                field: *field,
            }),
            ArcInstr::Construct {
                dst, ctor, args, ..
            } => {
                if matches!(
                    ctor,
                    ori_arc::CtorKind::MapLiteral
                        | ori_arc::CtorKind::SetLiteral
                        | ori_arc::CtorKind::Closure { .. }
                ) {
                    return Err(CompileError::UnsupportedConstructor {
                        function: function.name,
                        constructor: *ctor,
                    });
                }
                if !matches!(ctor, ori_arc::CtorKind::ListLiteral)
                    && args.len() > MAX_LOCAL_AGGREGATE_FIELDS
                {
                    return Err(CompileError::ConstructorTooWide {
                        function: function.name,
                        fields: args.len(),
                        limit: MAX_LOCAL_AGGREGATE_FIELDS,
                    });
                }
                Ok(Op::Construct {
                    dst: Register::from_arc(*dst),
                    ctor: *ctor,
                    args: self.add_operands(args)?,
                })
            }
            ArcInstr::RcInc { var, count, .. } => Ok(Op::RcInc {
                var: Register::from_arc(*var),
                count: *count,
            }),
            ArcInstr::RcDec { var, .. } => Ok(Op::RcDec {
                var: Register::from_arc(*var),
            }),
            ArcInstr::IsShared { dst, var } => Ok(Op::IsShared {
                dst: Register::from_arc(*dst),
                var: Register::from_arc(*var),
            }),
            ArcInstr::Set { base, field, value } => Ok(Op::Set {
                base: Register::from_arc(*base),
                field: *field,
                value: Register::from_arc(*value),
            }),
            ArcInstr::SetTag { base, tag } => Ok(Op::SetTag {
                base: Register::from_arc(*base),
                tag: *tag,
            }),
            ArcInstr::Select {
                dst,
                cond,
                true_val,
                false_val,
                ..
            } => Ok(Op::Select {
                dst: Register::from_arc(*dst),
                cond: Register::from_arc(*cond),
                true_value: Register::from_arc(*true_val),
                false_value: Register::from_arc(*false_val),
            }),
            other => Err(CompileError::UnsupportedInstruction {
                function: function.name,
                instruction: unsupported_kind(other),
            }),
        }
    }

    fn compile_let(
        &mut self,
        function: &ArcFunction,
        destination: ori_arc::ArcVarId,
        value: &ArcValue,
    ) -> Result<Op, CompileError> {
        let dst = Register::from_arc(destination);
        match value {
            ArcValue::Var(source) => Ok(Op::Copy {
                dst,
                src: Register::from_arc(*source),
            }),
            ArcValue::Literal(literal) => Ok(Op::Const {
                dst,
                value: self.compile_literal(function, *literal)?,
            }),
            ArcValue::PrimOp { op, args } => match (op, args.as_slice()) {
                (PrimOp::Binary(op), [left, right]) => {
                    Ok(self.compile_binary(function, dst, *op, *left, *right))
                }
                (PrimOp::Unary(op), [argument]) => {
                    Ok(self.compile_unary(function, dst, *op, *argument))
                }
                (PrimOp::Binary(_), operands) => Err(CompileError::PrimitiveArity {
                    function: function.name,
                    expected: 2,
                    actual: operands.len(),
                }),
                (PrimOp::Unary(_), operands) => Err(CompileError::PrimitiveArity {
                    function: function.name,
                    expected: 1,
                    actual: operands.len(),
                }),
            },
        }
    }

    fn compile_binary(
        &self,
        function: &ArcFunction,
        destination: Register,
        operation: ori_ir::BinaryOp,
        left: ori_arc::ArcVarId,
        right: ori_arc::ArcVarId,
    ) -> Op {
        let left_type = self.source.pool().builtin_type_tag(function.var_type(left));
        let right_type = self
            .source
            .pool()
            .builtin_type_tag(function.var_type(right));
        if self.options.typed_primitives
            && left_type == Some(BuiltinType::Int)
            && right_type == Some(BuiltinType::Int)
            && binary_primitive_strategy(BuiltinType::Int, operation) == PrimitiveStrategy::IntInstr
        {
            if let Some(operation) = IntBinaryOp::from_binary(operation) {
                return Op::IntBinary {
                    dst: destination,
                    op: operation,
                    lhs: Register::from_arc(left),
                    rhs: Register::from_arc(right),
                };
            }
        }
        if self.options.typed_primitives
            && left_type == Some(BuiltinType::Str)
            && right_type == Some(BuiltinType::Str)
            && matches!(
                binary_primitive_strategy(BuiltinType::Str, operation),
                PrimitiveStrategy::RuntimeCall { .. }
            )
        {
            if let Some(operation) = StringBinaryOp::from_binary(operation) {
                return Op::StringBinary {
                    dst: destination,
                    op: operation,
                    lhs: Register::from_arc(left),
                    rhs: Register::from_arc(right),
                };
            }
        }
        Op::Binary {
            dst: destination,
            op: operation,
            lhs: Register::from_arc(left),
            rhs: Register::from_arc(right),
        }
    }

    fn compile_unary(
        &self,
        function: &ArcFunction,
        destination: Register,
        operation: ori_ir::UnaryOp,
        argument: ori_arc::ArcVarId,
    ) -> Op {
        let argument_type = self
            .source
            .pool()
            .builtin_type_tag(function.var_type(argument));
        if self.options.typed_primitives
            && operation == ori_ir::UnaryOp::Not
            && argument_type == Some(BuiltinType::Bool)
            && unary_primitive_strategy(BuiltinType::Bool, operation)
                == PrimitiveStrategy::BoolLogic
        {
            return Op::BoolNot {
                dst: destination,
                arg: Register::from_arc(argument),
            };
        }
        Op::Unary {
            dst: destination,
            op: operation,
            arg: Register::from_arc(argument),
        }
    }

    fn compile_literal(
        &mut self,
        function: &ArcFunction,
        literal: LitValue,
    ) -> Result<Constant, CompileError> {
        match literal {
            LitValue::Int(value) => Ok(Constant::Int(value)),
            LitValue::Float(bits) => Ok(Constant::Float(bits)),
            LitValue::Bool(value) => Ok(Constant::Bool(value)),
            LitValue::String(name) => self
                .add_string(self.source.symbols().lookup(name).to_owned())
                .map(Constant::String),
            LitValue::Char(value) => Ok(Constant::Char(value)),
            LitValue::Unit => Ok(Constant::Unit),
            LitValue::Null => Ok(Constant::Null),
            LitValue::Duration { value, .. } | LitValue::Size { value, .. } => i64::try_from(value)
                .map(Constant::Int)
                .map_err(|_| CompileError::LiteralOutOfRange {
                    function: function.name,
                }),
        }
    }

    fn call_target(
        &self,
        function: &ArcFunction,
        block_index: usize,
        instruction_index: usize,
    ) -> Result<ori_repr::executable::CallableTarget, CompileError> {
        let function_id =
            self.source
                .function_id(function.name)
                .ok_or(CompileError::MissingCallTarget {
                    function: function.name,
                    block: block_index,
                    position: instruction_index,
                })?;
        let block = BlockIndex::new(block_index, function.name).map_err(|_| {
            CompileError::FunctionTooLarge {
                function: function.name,
                count: block_index,
            }
        })?;
        let position =
            CallPosition::instruction(instruction_index, function.name).map_err(|_| {
                CompileError::FunctionTooLarge {
                    function: function.name,
                    count: instruction_index,
                }
            })?;
        self.source
            .call_target(CallSite::new(function_id, block, position))
            .ok_or(CompileError::MissingCallTarget {
                function: function.name,
                block: block_index,
                position: instruction_index,
            })
    }
}

fn unsupported_kind(instruction: &ArcInstr) -> ArcInstructionKind {
    match instruction {
        ArcInstr::ApplyIndirect { .. } => ArcInstructionKind::ApplyIndirect,
        ArcInstr::PartialApply { .. } => ArcInstructionKind::PartialApply,
        ArcInstr::RcDecPartial { .. } => ArcInstructionKind::RcDecPartial,
        ArcInstr::RcDecField { .. } => ArcInstructionKind::RcDecField,
        ArcInstr::RcDecVariant { .. } => ArcInstructionKind::RcDecVariant,
        ArcInstr::BurdenInc { .. } => ArcInstructionKind::BurdenInc,
        ArcInstr::BurdenDec { .. } => ArcInstructionKind::BurdenDec,
        ArcInstr::BurdenDecPartial { .. } => ArcInstructionKind::BurdenDecPartial,
        ArcInstr::BurdenDecField { .. } => ArcInstructionKind::BurdenDecField,
        ArcInstr::BurdenDecVariant { .. } => ArcInstructionKind::BurdenDecVariant,
        ArcInstr::Reset { .. } => ArcInstructionKind::Reset,
        ArcInstr::Reuse { .. } => ArcInstructionKind::Reuse,
        ArcInstr::CollectionReuse { .. } => ArcInstructionKind::CollectionReuse,
        ArcInstr::Let { .. }
        | ArcInstr::Apply { .. }
        | ArcInstr::Project { .. }
        | ArcInstr::Construct { .. }
        | ArcInstr::RcInc { .. }
        | ArcInstr::RcDec { .. }
        | ArcInstr::IsShared { .. }
        | ArcInstr::Set { .. }
        | ArcInstr::SetTag { .. }
        | ArcInstr::Select { .. } => {
            unreachable!("supported instructions return before classification")
        }
    }
}
