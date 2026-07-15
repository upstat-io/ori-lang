//! ARC instruction lowering.

use ori_arc::{
    ArcFunction, ArcInstr, ArcValue, ArcVarId, ArgOwnership, LitValue, PrimOp, RcAtomicity,
    RcStrategy,
};
use ori_registry::{OpStrategy, RuntimeOperator};
use ori_repr::executable::{BlockIndex, CallPosition, CallSite};

use super::Compiler;
use crate::bytecode::{
    Constant, Continuation, IntBinaryOp, Op, RcSemantics, Register, StringBinaryOp,
};
use crate::{ArcInstructionKind, CompileError};

const MAX_LOCAL_AGGREGATE_FIELDS: usize = 4;

impl Compiler<'_> {
    pub(super) fn compile_instruction(
        &mut self,
        function: &ArcFunction,
        block_index: usize,
        instruction_index: usize,
        instruction: &ArcInstr,
        register_rc_strategies: &[Option<RcStrategy>],
    ) -> Result<Op, CompileError> {
        match instruction {
            ArcInstr::Let { dst, value, .. } => self.compile_let(function, *dst, value),
            ArcInstr::Apply {
                dst,
                args,
                arg_ownership,
                ..
            } => self.compile_direct_call(
                function,
                block_index,
                instruction_index,
                *dst,
                args,
                arg_ownership,
            ),
            ArcInstr::ApplyIndirect {
                dst,
                closure,
                args,
                arg_ownership,
                ..
            } => self.compile_closure_call(function, *dst, *closure, args, arg_ownership),
            ArcInstr::PartialApply { dst, args, .. } => {
                self.compile_partial_apply(function, block_index, instruction_index, *dst, args)
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
            } => self.compile_construct(function, *dst, *ctor, args),
            ArcInstr::RcInc {
                var,
                count,
                strategy,
                atomicity,
            } => compile_rc_inc(
                function,
                *var,
                *count,
                *strategy,
                *atomicity,
                register_rc_strategies,
            ),
            ArcInstr::RcDec {
                var,
                strategy,
                atomicity,
            } => compile_rc_dec(
                function,
                *var,
                *strategy,
                *atomicity,
                register_rc_strategies,
            ),
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
            other => {
                let instruction = unsupported_kind(other);
                Err(CompileError::UnsupportedInstruction {
                    function: function.name,
                    function_symbol: self.source.symbols().lookup(function.name).into(),
                    instruction,
                    operation: unsupported_operation(instruction),
                })
            }
        }
    }

    fn compile_direct_call(
        &mut self,
        function: &ArcFunction,
        block_index: usize,
        instruction_index: usize,
        destination: ArcVarId,
        arguments: &[ArcVarId],
        ownership: &[ArgOwnership],
    ) -> Result<Op, CompileError> {
        let callee = super::validate_vm_call_target(
            function.name,
            self.call_target(function, block_index, instruction_index)?,
        )?;
        Ok(Op::Call {
            dst: Register::from_arc(destination),
            callee,
            args: self.add_call_arguments(function.name, arguments, ownership)?,
            normal: Continuation::Next,
            unwind: None,
        })
    }

    fn compile_closure_call(
        &mut self,
        function: &ArcFunction,
        destination: ArcVarId,
        closure: ArcVarId,
        arguments: &[ArcVarId],
        ownership: &[ArgOwnership],
    ) -> Result<Op, CompileError> {
        Ok(Op::CallClosure {
            dst: Register::from_arc(destination),
            closure: Register::from_arc(closure),
            args: self.add_call_arguments(function.name, arguments, ownership)?,
            normal: Continuation::Next,
            unwind: None,
        })
    }

    fn compile_partial_apply(
        &mut self,
        function: &ArcFunction,
        block_index: usize,
        instruction_index: usize,
        destination: ArcVarId,
        captures: &[ArcVarId],
    ) -> Result<Op, CompileError> {
        let target = self.call_target(function, block_index, instruction_index)?;
        let ori_repr::executable::CallableTarget::Function(callee) = target else {
            return Err(CompileError::InvalidClosureTarget {
                function: function.name,
                target,
            });
        };
        Ok(Op::MakeClosure {
            dst: Register::from_arc(destination),
            callee,
            captures: self.add_operands(captures)?,
        })
    }

    fn compile_construct(
        &mut self,
        function: &ArcFunction,
        destination: ori_arc::ArcVarId,
        constructor: ori_arc::CtorKind,
        arguments: &[ori_arc::ArcVarId],
    ) -> Result<Op, CompileError> {
        if matches!(
            constructor,
            ori_arc::CtorKind::MapLiteral
                | ori_arc::CtorKind::SetLiteral
                | ori_arc::CtorKind::Closure { .. }
        ) {
            return Err(CompileError::UnsupportedConstructor {
                function: function.name,
                constructor,
            });
        }
        if !matches!(constructor, ori_arc::CtorKind::ListLiteral)
            && arguments.len() > MAX_LOCAL_AGGREGATE_FIELDS
        {
            return Err(CompileError::ConstructorTooWide {
                function: function.name,
                fields: arguments.len(),
                limit: MAX_LOCAL_AGGREGATE_FIELDS,
            });
        }
        Ok(Op::Construct {
            dst: Register::from_arc(destination),
            ctor: constructor,
            args: self.add_operands(arguments)?,
        })
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
                    let fact = primitive_fact(function, destination, args.len())?;
                    self.compile_binary(function.name, dst, *op, *left, *right, fact.strategy)
                }
                (PrimOp::Unary(op), [argument]) => {
                    let fact = primitive_fact(function, destination, args.len())?;
                    Ok(self.compile_unary(dst, *op, *argument, fact.strategy))
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
        function: ori_ir::Name,
        destination: Register,
        operation: ori_ir::BinaryOp,
        left: ori_arc::ArcVarId,
        right: ori_arc::ArcVarId,
        strategy: OpStrategy,
    ) -> Result<Op, CompileError> {
        if matches!(
            strategy,
            OpStrategy::StructuralEquality
                | OpStrategy::StructuralOrdering
                | OpStrategy::Unsupported
        ) {
            return Err(CompileError::UnsupportedPrimitiveProjection {
                function,
                destination: destination.index(),
                strategy,
            });
        }
        if self.options.typed_primitives && strategy == OpStrategy::SignedInteger {
            if let Some(operation) = IntBinaryOp::from_binary(operation) {
                return Ok(Op::IntBinary {
                    dst: destination,
                    op: operation,
                    lhs: Register::from_arc(left),
                    rhs: Register::from_arc(right),
                });
            }
        }
        if self.options.typed_primitives
            && matches!(
                strategy,
                OpStrategy::RuntimeCall(
                    RuntimeOperator::StringConcat
                        | RuntimeOperator::StringEqual
                        | RuntimeOperator::StringNotEqual
                        | RuntimeOperator::StringCompare
                )
            )
        {
            if let Some(operation) = StringBinaryOp::from_binary(operation) {
                return Ok(Op::StringBinary {
                    dst: destination,
                    op: operation,
                    lhs: Register::from_arc(left),
                    rhs: Register::from_arc(right),
                });
            }
        }
        if strategy == OpStrategy::RuntimeCall(RuntimeOperator::ListConcat) {
            return Ok(Op::RuntimeBinary {
                dst: destination,
                operator: RuntimeOperator::ListConcat,
                lhs: Register::from_arc(left),
                rhs: Register::from_arc(right),
            });
        }
        Ok(Op::Binary {
            dst: destination,
            op: operation,
            lhs: Register::from_arc(left),
            rhs: Register::from_arc(right),
        })
    }

    fn compile_unary(
        &self,
        destination: Register,
        operation: ori_ir::UnaryOp,
        argument: ori_arc::ArcVarId,
        strategy: OpStrategy,
    ) -> Op {
        if self.options.typed_primitives
            && operation == ori_ir::UnaryOp::Not
            && strategy == OpStrategy::BooleanLogic
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

fn primitive_fact(
    function: &ArcFunction,
    destination: ori_arc::ArcVarId,
    arity: usize,
) -> Result<ori_arc::ir::PrimitiveFact, CompileError> {
    let fact =
        function
            .primitive_facts
            .get(destination)
            .ok_or(CompileError::MissingPrimitiveFact {
                function: function.name,
                destination: destination.index(),
            })?;
    if fact.is_valid_for(arity) {
        Ok(fact)
    } else {
        Err(CompileError::InvalidPrimitiveFact {
            function: function.name,
            destination: destination.index(),
        })
    }
}

fn compile_rc_inc(
    function: &ArcFunction,
    var: ori_arc::ArcVarId,
    count: u32,
    strategy: RcStrategy,
    atomicity: RcAtomicity,
    register_rc_strategies: &[Option<RcStrategy>],
) -> Result<Op, CompileError> {
    Ok(Op::RcInc {
        var: Register::from_arc(var),
        count,
        semantics: rc_semantics(function, var, strategy, atomicity, register_rc_strategies)?,
    })
}

fn compile_rc_dec(
    function: &ArcFunction,
    var: ori_arc::ArcVarId,
    strategy: RcStrategy,
    atomicity: RcAtomicity,
    register_rc_strategies: &[Option<RcStrategy>],
) -> Result<Op, CompileError> {
    Ok(Op::RcDec {
        var: Register::from_arc(var),
        semantics: rc_semantics(function, var, strategy, atomicity, register_rc_strategies)?,
    })
}

fn rc_semantics(
    function: &ArcFunction,
    var: ori_arc::ArcVarId,
    strategy: RcStrategy,
    atomicity: RcAtomicity,
    register_rc_strategies: &[Option<RcStrategy>],
) -> Result<RcSemantics, CompileError> {
    let semantics = RcSemantics::new(strategy, atomicity);
    if !semantics.has_supported_atomicity() {
        return Err(CompileError::UnsupportedRcAtomicity {
            function: function.name,
            atomicity,
        });
    }
    if !semantics.has_supported_strategy() {
        return Err(CompileError::UnsupportedRcStrategy {
            function: function.name,
            strategy,
        });
    }
    let expected = register_rc_strategies.get(var.index()).copied().flatten();
    if expected != Some(strategy) {
        return Err(CompileError::RcStrategyMismatch {
            function: function.name,
            register: var.index(),
            expected,
            found: strategy,
        });
    }
    Ok(semantics)
}

fn unsupported_kind(instruction: &ArcInstr) -> ArcInstructionKind {
    match instruction {
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
        | ArcInstr::ApplyIndirect { .. }
        | ArcInstr::PartialApply { .. }
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

const fn unsupported_operation(instruction: ArcInstructionKind) -> &'static str {
    match instruction {
        ArcInstructionKind::RcDecPartial => "partial-value reference-count cleanup",
        ArcInstructionKind::RcDecField => "field reference-count cleanup",
        ArcInstructionKind::RcDecVariant => "variant reference-count cleanup",
        ArcInstructionKind::BurdenInc => "an unresolved ownership increment",
        ArcInstructionKind::BurdenDec => "an unresolved ownership decrement",
        ArcInstructionKind::BurdenDecPartial => "partial-value ownership cleanup",
        ArcInstructionKind::BurdenDecField => "field ownership cleanup",
        ArcInstructionKind::BurdenDecVariant => "variant ownership cleanup",
        ArcInstructionKind::Reset => "a constructor-reuse reset",
        ArcInstructionKind::Reuse => "constructor allocation reuse",
        ArcInstructionKind::CollectionReuse => "collection-buffer reuse",
    }
}
