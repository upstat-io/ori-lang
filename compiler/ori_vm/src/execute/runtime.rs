//! Backend-neutral runtime-call dispatch for VM values.

mod collections;
mod dispatch;
mod iterators;
mod list_mutations;
mod strings;

#[cfg(test)]
mod list_mutation_tests;
#[cfg(test)]
mod test_support;
#[cfg(test)]
mod tests;

use ori_arc::ArgOwnership;
use ori_repr::executable::RuntimeCall;

use crate::ExecutionError;

use super::operands::OperandAccess;
use super::value::VmValue;
use super::Interpreter;

impl Interpreter<'_> {
    fn runtime_values<const N: usize>(
        &self,
        frame: usize,
        operands: usize,
        call: RuntimeCall,
        operand_cursor: &mut impl OperandAccess,
    ) -> Result<[VmValue; N], ExecutionError> {
        let arguments = self.call_arguments(operands);
        let actual = arguments.len();
        let arguments: &[crate::bytecode::CallArgument; N] =
            arguments
                .try_into()
                .map_err(|_| ExecutionError::RuntimeArity {
                    call,
                    expected: call.arity(),
                    actual,
                })?;
        Ok(std::array::from_fn(|index| {
            operand_cursor.read(&self.frames[frame].registers, arguments[index].register())
        }))
    }

    fn runtime_argument_ownership(
        &self,
        operands: usize,
        index: usize,
        call: RuntimeCall,
    ) -> Result<ArgOwnership, ExecutionError> {
        self.call_arguments(operands)
            .get(index)
            .copied()
            .map(crate::bytecode::CallArgument::ownership)
            .ok_or(ExecutionError::RuntimeArity {
                call,
                expected: call.arity(),
                actual: self.call_arguments(operands).len(),
            })
    }
}
