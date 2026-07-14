//! Structural verification for compiled bytecode.

use ori_ir::Name;
use ori_repr::executable::CallableTarget;

use crate::bytecode::{
    BytecodeFunction, BytecodeProgram, Constant, Continuation, Op, Pc, Register, RegisterClass,
    VerifiedProgram,
};
use crate::{IndexKind, VerifyError};

/// Verify every bytecode reference and consume the unverified artifact.
pub fn verify(program: BytecodeProgram) -> Result<VerifiedProgram, VerifyError> {
    let main = program.functions.get(program.main.index()).ok_or_else(|| {
        invalid_index(
            Name::EMPTY,
            None,
            IndexKind::Function,
            program.main.index(),
            program.functions.len(),
        )
    })?;
    if !main.params.is_empty() {
        return Err(VerifyError::MainHasParameters {
            function: main.name,
            parameters: main.params.len(),
        });
    }

    for function in &program.functions {
        Verifier::new(&program, function).verify()?;
    }
    Ok(VerifiedProgram { program })
}

struct Verifier<'a> {
    program: &'a BytecodeProgram,
    function: &'a BytecodeFunction,
}

impl<'a> Verifier<'a> {
    const fn new(program: &'a BytecodeProgram, function: &'a BytecodeFunction) -> Self {
        Self { program, function }
    }

    fn verify(&self) -> Result<(), VerifyError> {
        if self.function.register_classes.len() != self.function.register_count {
            return Err(VerifyError::RegisterMetadata {
                function: self.function.name,
                registers: self.function.register_count,
                classes: self.function.register_classes.len(),
            });
        }
        self.pc(None, self.function.entry)?;
        for &parameter in &self.function.params {
            self.register(None, parameter)?;
        }
        for (pc, operation) in self.function.ops.iter().enumerate() {
            self.operation(pc, *operation)?;
        }
        Ok(())
    }

    fn operation(&self, pc: usize, operation: Op) -> Result<(), VerifyError> {
        if operation.needs_next_pc() && pc + 1 >= self.function.ops.len() {
            return Err(VerifyError::InvalidFallthrough {
                function: self.function.name,
                pc,
            });
        }
        match operation {
            Op::Const { dst, value } => self.constant(pc, dst, value)?,
            Op::Copy { dst, src } => self.registers(pc, &[dst, src])?,
            Op::Binary { dst, lhs, rhs, .. } => self.registers(pc, &[dst, lhs, rhs])?,
            Op::IntBinary { dst, op, lhs, rhs } => {
                self.typed_binary(
                    pc,
                    dst,
                    lhs,
                    rhs,
                    RegisterClass::Int,
                    if op.returns_bool() {
                        RegisterClass::Bool
                    } else {
                        RegisterClass::Int
                    },
                )?;
            }
            Op::StringBinary { dst, op, lhs, rhs } => {
                self.typed_binary(
                    pc,
                    dst,
                    lhs,
                    rhs,
                    RegisterClass::String,
                    if op.returns_bool() {
                        RegisterClass::Bool
                    } else {
                        RegisterClass::String
                    },
                )?;
            }
            Op::Unary { dst, arg, .. } => self.registers(pc, &[dst, arg])?,
            Op::BoolNot { dst, arg } => {
                self.typed_register(pc, dst, RegisterClass::Bool)?;
                self.typed_register(pc, arg, RegisterClass::Bool)?;
            }
            Op::Call {
                dst,
                callee,
                args,
                normal,
                unwind,
            } => self.call(pc, dst, callee, args.index(), normal, unwind)?,
            Op::Construct { dst, args, .. } => {
                self.register(Some(pc), dst)?;
                self.operand_registers(pc, args.index())?;
            }
            Op::Project { dst, value, .. } => self.registers(pc, &[dst, value])?,
            Op::RcInc { var, .. } | Op::RcDec { var } => {
                self.register(Some(pc), var)?;
            }
            Op::IsShared { dst, var } => self.registers(pc, &[dst, var])?,
            Op::Set { base, value, .. } => self.registers(pc, &[base, value])?,
            Op::SetTag { base, .. } => self.register(Some(pc), base)?,
            Op::Select {
                dst,
                cond,
                true_value,
                false_value,
            } => self.registers(pc, &[dst, cond, true_value, false_value])?,
            Op::Jump { target, moves } => {
                self.pc(Some(pc), target)?;
                let entries = self.moves(pc, moves.index())?;
                for &(destination, source) in entries {
                    self.registers(pc, &[destination, source])?;
                }
            }
            Op::Branch {
                cond,
                then_pc,
                else_pc,
            } => {
                self.register(Some(pc), cond)?;
                self.pc(Some(pc), then_pc)?;
                self.pc(Some(pc), else_pc)?;
            }
            Op::Switch {
                scrutinee,
                table,
                default_pc,
            } => {
                self.register(Some(pc), scrutinee)?;
                self.pc(Some(pc), default_pc)?;
                let cases = self.switches(pc, table.index())?;
                for &(_, target) in cases {
                    self.pc(Some(pc), target)?;
                }
            }
            Op::Return { value } => self.register(Some(pc), value)?,
            Op::Resume | Op::Unreachable => {}
        }
        Ok(())
    }

    fn constant(
        &self,
        pc: usize,
        destination: Register,
        value: Constant,
    ) -> Result<(), VerifyError> {
        self.register(Some(pc), destination)?;
        if let Constant::String(string) = value {
            self.index(
                Some(pc),
                IndexKind::String,
                string.index(),
                self.program.strings.len(),
            )?;
        }
        Ok(())
    }

    fn typed_binary(
        &self,
        pc: usize,
        destination: Register,
        left: Register,
        right: Register,
        operand_class: RegisterClass,
        result_class: RegisterClass,
    ) -> Result<(), VerifyError> {
        self.typed_register(pc, left, operand_class)?;
        self.typed_register(pc, right, operand_class)?;
        self.typed_register(pc, destination, result_class)
    }

    fn call(
        &self,
        pc: usize,
        destination: Register,
        target: CallableTarget,
        operands: usize,
        normal: Continuation,
        unwind: Option<Pc>,
    ) -> Result<(), VerifyError> {
        self.register(Some(pc), destination)?;
        let arguments = self.operand_registers(pc, operands)?;
        let expected = match target {
            CallableTarget::Function(function) => self
                .program
                .functions
                .get(function.index())
                .ok_or_else(|| {
                    invalid_index(
                        self.function.name,
                        Some(pc),
                        IndexKind::Function,
                        function.index(),
                        self.program.functions.len(),
                    )
                })?
                .params
                .len(),
            CallableTarget::Runtime(call) => call.arity(),
        };
        if arguments.len() != expected {
            return Err(VerifyError::CallArity {
                function: self.function.name,
                pc,
                target,
                expected,
                actual: arguments.len(),
            });
        }
        if let Continuation::At(target) = normal {
            self.pc(Some(pc), target)?;
        }
        if let Some(target) = unwind {
            self.pc(Some(pc), target)?;
        }
        Ok(())
    }

    fn operand_registers(&self, pc: usize, index: usize) -> Result<&'a [Register], VerifyError> {
        let operands = self.program.operands.get(index).ok_or_else(|| {
            invalid_index(
                self.function.name,
                Some(pc),
                IndexKind::Operands,
                index,
                self.program.operands.len(),
            )
        })?;
        for &register in operands {
            self.register(Some(pc), register)?;
        }
        Ok(operands)
    }

    fn moves(&self, pc: usize, index: usize) -> Result<&'a [(Register, Register)], VerifyError> {
        self.program
            .moves
            .get(index)
            .map(AsRef::as_ref)
            .ok_or_else(|| {
                invalid_index(
                    self.function.name,
                    Some(pc),
                    IndexKind::Moves,
                    index,
                    self.program.moves.len(),
                )
            })
    }

    fn switches(&self, pc: usize, index: usize) -> Result<&'a [(u64, Pc)], VerifyError> {
        self.program
            .switches
            .get(index)
            .map(AsRef::as_ref)
            .ok_or_else(|| {
                invalid_index(
                    self.function.name,
                    Some(pc),
                    IndexKind::Switch,
                    index,
                    self.program.switches.len(),
                )
            })
    }

    fn registers(&self, pc: usize, registers: &[Register]) -> Result<(), VerifyError> {
        for &register in registers {
            self.register(Some(pc), register)?;
        }
        Ok(())
    }

    fn register(&self, pc: Option<usize>, register: Register) -> Result<(), VerifyError> {
        self.index(
            pc,
            IndexKind::Register,
            register.index(),
            self.function.register_count,
        )
    }

    fn typed_register(
        &self,
        pc: usize,
        register: Register,
        expected: RegisterClass,
    ) -> Result<(), VerifyError> {
        self.register(Some(pc), register)?;
        let found = self.function.register_classes[register.index()];
        if found == expected {
            Ok(())
        } else {
            Err(VerifyError::TypedRegister {
                function: self.function.name,
                pc,
                register: register.index(),
                expected: expected.name(),
                found: found.name(),
            })
        }
    }

    fn pc(&self, source: Option<usize>, target: Pc) -> Result<(), VerifyError> {
        self.index(
            source,
            IndexKind::ProgramCounter,
            target.index(),
            self.function.ops.len(),
        )
    }

    fn index(
        &self,
        pc: Option<usize>,
        kind: IndexKind,
        index: usize,
        bound: usize,
    ) -> Result<(), VerifyError> {
        if index < bound {
            Ok(())
        } else {
            Err(invalid_index(self.function.name, pc, kind, index, bound))
        }
    }
}

fn invalid_index(
    function: Name,
    pc: Option<usize>,
    kind: IndexKind,
    index: usize,
    bound: usize,
) -> VerifyError {
    VerifyError::InvalidIndex {
        function,
        pc,
        kind,
        index,
        bound,
    }
}
