//! Canonical traversal of register operands embedded in bytecode.

use super::{
    BytecodeProgram, CallArgumentListId, MoveListId, Op, OperandListId, Register, TableKind,
};

/// Whether a bytecode operand reads or defines a register.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RegisterOperand {
    Read(Register),
    Write(Register),
}

/// A side-table reference required to enumerate an operation's registers is invalid.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct OperandTableError {
    pub(crate) table: TableKind,
    pub(crate) index: usize,
    pub(crate) bound: usize,
}

/// Visit every register operand once in its canonical semantic order.
///
/// Call arguments remain ownership-bearing entries in the bytecode table. This
/// walker projects only their register identities for consumers that do not own
/// call semantics.
pub(crate) fn walk_register_operands(
    program: &BytecodeProgram,
    operation: Op,
    mut visit: impl FnMut(RegisterOperand),
) -> Result<(), OperandTableError> {
    use RegisterOperand::{Read, Write};

    match operation {
        Op::Const { dst, .. } => visit(Write(dst)),
        Op::Copy { dst, src } => {
            visit(Write(dst));
            visit(Read(src));
        }
        Op::Binary { dst, lhs, rhs, .. }
        | Op::IntBinary { dst, lhs, rhs, .. }
        | Op::StringBinary { dst, lhs, rhs, .. }
        | Op::RuntimeBinary { dst, lhs, rhs, .. } => {
            visit(Write(dst));
            visit(Read(lhs));
            visit(Read(rhs));
        }
        Op::Unary { dst, arg, .. } | Op::BoolNot { dst, arg } => {
            visit(Write(dst));
            visit(Read(arg));
        }
        Op::Call { dst, args, .. } => {
            visit(Write(dst));
            walk_call_arguments(program, args, &mut visit)?;
        }
        Op::MakeClosure { dst, captures, .. } => {
            visit(Write(dst));
            walk_operand_list(program, captures, &mut visit)?;
        }
        Op::CallClosure {
            dst, closure, args, ..
        } => {
            visit(Write(dst));
            visit(Read(closure));
            walk_call_arguments(program, args, &mut visit)?;
        }
        Op::Construct { dst, args, .. } => {
            visit(Write(dst));
            walk_operand_list(program, args, &mut visit)?;
        }
        Op::Project { dst, value, .. } => {
            visit(Write(dst));
            visit(Read(value));
        }
        Op::RcInc { var, .. } | Op::RcDec { var, .. } => visit(Read(var)),
        Op::IsShared { dst, var } => {
            visit(Write(dst));
            visit(Read(var));
        }
        Op::Set { base, value, .. } => {
            visit(Read(base));
            visit(Read(value));
        }
        Op::SetTag { base, .. } => visit(Read(base)),
        Op::Select {
            dst,
            cond,
            true_value,
            false_value,
        } => {
            visit(Write(dst));
            visit(Read(cond));
            visit(Read(true_value));
            visit(Read(false_value));
        }
        Op::Jump { moves, .. } => {
            walk_moves(program, moves, &mut visit)?;
        }
        Op::Branch { cond, .. } => visit(Read(cond)),
        Op::Switch { scrutinee, .. } => visit(Read(scrutinee)),
        Op::Return { value } => visit(Read(value)),
        Op::Resume | Op::Unreachable => {}
    }
    Ok(())
}

fn walk_call_arguments(
    program: &BytecodeProgram,
    arguments: CallArgumentListId,
    visit: &mut impl FnMut(RegisterOperand),
) -> Result<(), OperandTableError> {
    let arguments = table_entry(
        &program.call_arguments,
        TableKind::CallArguments,
        arguments.index(),
    )?;
    for argument in arguments {
        visit(RegisterOperand::Read(argument.register()));
    }
    Ok(())
}

fn walk_operand_list(
    program: &BytecodeProgram,
    operands: OperandListId,
    visit: &mut impl FnMut(RegisterOperand),
) -> Result<(), OperandTableError> {
    let operands = table_entry(&program.operands, TableKind::Operands, operands.index())?;
    for &register in operands {
        visit(RegisterOperand::Read(register));
    }
    Ok(())
}

fn walk_moves(
    program: &BytecodeProgram,
    moves: MoveListId,
    visit: &mut impl FnMut(RegisterOperand),
) -> Result<(), OperandTableError> {
    let moves = table_entry(&program.moves, TableKind::Moves, moves.index())?;
    for &(destination, source) in moves {
        visit(RegisterOperand::Write(destination));
        visit(RegisterOperand::Read(source));
    }
    Ok(())
}

fn table_entry<T>(table: &[T], kind: TableKind, index: usize) -> Result<&T, OperandTableError> {
    table.get(index).ok_or(OperandTableError {
        table: kind,
        index,
        bound: table.len(),
    })
}
