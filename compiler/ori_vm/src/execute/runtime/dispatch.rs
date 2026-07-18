//! Canonical runtime-call routing over selected operand storage.

mod collections;
mod iterators;
mod strings;

use ori_repr::executable::RuntimeCall;

use crate::ExecutionError;

use super::super::operands::{FrameSlot, OperandAccess};
use super::super::value::VmValue;
use super::super::Interpreter;

#[derive(Clone, Copy, Debug)]
pub(super) struct RuntimeSite {
    frame: usize,
    destination: FrameSlot,
    operands: usize,
    call: RuntimeCall,
}

impl Interpreter<'_> {
    pub(in crate::execute) fn execute_runtime(
        &mut self,
        frame: usize,
        destination: FrameSlot,
        call: RuntimeCall,
        operands: usize,
        operand_cursor: &mut impl OperandAccess,
    ) -> Result<VmValue, ExecutionError> {
        let site = RuntimeSite {
            frame,
            destination,
            operands,
            call,
        };
        match call {
            RuntimeCall::Iter(source) => self.runtime_iter(site, source, operand_cursor),
            RuntimeCall::IterNext => self.runtime_iter_next(site, operand_cursor),
            RuntimeCall::IterDrop => self.runtime_iter_drop(site, operand_cursor),
            RuntimeCall::ListNew => self.runtime_list_new(site, operand_cursor),
            RuntimeCall::ListFree => self.runtime_list_free(site, operand_cursor),
            RuntimeCall::ListBuilderPush => self.runtime_list_builder_push(site, operand_cursor),
            RuntimeCall::ListPush => self.runtime_list_push(site, operand_cursor),
            RuntimeCall::ListInsert => self.runtime_list_insert(site, operand_cursor),
            RuntimeCall::ListRemove => self.runtime_list_remove(site, operand_cursor),
            RuntimeCall::ListPrepend => self.runtime_list_prepend(site, operand_cursor),
            RuntimeCall::ListTake => self.runtime_list_take(site, operand_cursor),
            RuntimeCall::Index => self.runtime_index(site, operand_cursor),
            RuntimeCall::ListSet => self.runtime_list_set(site, operand_cursor),
            RuntimeCall::Length => self.runtime_length(site, operand_cursor),
            RuntimeCall::ToString => self.runtime_to_string(site, operand_cursor),
            RuntimeCall::Concat => self.runtime_concat(site, operand_cursor),
            RuntimeCall::StringContains => self.runtime_string_contains(site, operand_cursor),
            RuntimeCall::StringStartsWith => self.runtime_string_starts_with(site, operand_cursor),
            RuntimeCall::StringEndsWith => self.runtime_string_ends_with(site, operand_cursor),
            RuntimeCall::StringIsEmpty => self.runtime_string_is_empty(site, operand_cursor),
            RuntimeCall::StringTrim => self.runtime_string_trim(site, operand_cursor),
            RuntimeCall::StringUppercase => self.runtime_string_uppercase(site, operand_cursor),
            RuntimeCall::StringLowercase => self.runtime_string_lowercase(site, operand_cursor),
            RuntimeCall::StringSplit => self.runtime_string_split(site, operand_cursor),
            RuntimeCall::RegisteredMethod(_)
            | RuntimeCall::RegistryMethod(_)
            | RuntimeCall::RegistryPrelude(_)
            | RuntimeCall::Protocol(_)
            | RuntimeCall::Compiler(_) => Err(ExecutionError::UnsupportedRuntimeCall { call }),
            RuntimeCall::Print => self.runtime_print(site, operand_cursor),
            RuntimeCall::Panic => self.runtime_panic(site, operand_cursor),
        }
    }
}
