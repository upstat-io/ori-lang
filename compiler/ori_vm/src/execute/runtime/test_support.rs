//! Register-oriented adapters for runtime unit tests.

use ori_arc::ArgOwnership;
use ori_repr::executable::IteratorSource;

use crate::bytecode::Register;
use crate::ExecutionError;

use super::super::value::VmValue;
use super::super::Interpreter;

impl Interpreter<'_> {
    pub(super) fn length(&self, frame: usize, value: Register) -> Result<VmValue, ExecutionError> {
        self.length_value(self.register(frame, value))
    }

    pub(super) fn string_split(
        &mut self,
        frame: usize,
        value: Register,
        separator: Register,
    ) -> Result<VmValue, ExecutionError> {
        let value = self.register(frame, value);
        let separator = self.register(frame, separator);
        self.string_split_value(value, separator)
    }

    pub(super) fn create_iterator(
        &mut self,
        frame: usize,
        destination: Register,
        source_kind: IteratorSource,
        source: Register,
        ownership: ArgOwnership,
    ) -> Result<VmValue, ExecutionError> {
        let function = self.frames[frame].function;
        let destination = self.layout.frame_slot(function, destination);
        let source = self.register(frame, source);
        self.create_iterator_value(frame, destination, source_kind, source, ownership)
    }

    pub(super) fn iter_next(
        &mut self,
        frame: usize,
        iterator: Register,
    ) -> Result<VmValue, ExecutionError> {
        let iterator = self.register(frame, iterator);
        self.iter_next_value(iterator)
    }

    pub(super) fn list_push(
        &mut self,
        frame: usize,
        list: Register,
        value: Register,
        list_ownership: ArgOwnership,
        value_ownership: ArgOwnership,
    ) -> Result<VmValue, ExecutionError> {
        self.list_push_value(
            self.register(frame, list),
            self.register(frame, value),
            list_ownership,
            value_ownership,
        )
    }

    pub(super) fn list_concat(
        &mut self,
        frame: usize,
        left: Register,
        right: Register,
    ) -> Result<VmValue, ExecutionError> {
        self.list_concat_value(self.register(frame, left), self.register(frame, right))
    }

    pub(super) fn list_insert(
        &mut self,
        frame: usize,
        list: Register,
        index: Register,
        value: Register,
        list_ownership: ArgOwnership,
        value_ownership: ArgOwnership,
    ) -> Result<VmValue, ExecutionError> {
        self.list_insert_value(
            self.register(frame, list),
            self.register(frame, index),
            self.register(frame, value),
            list_ownership,
            value_ownership,
        )
    }

    pub(super) fn list_prepend(
        &mut self,
        frame: usize,
        list: Register,
        value: Register,
        list_ownership: ArgOwnership,
        value_ownership: ArgOwnership,
    ) -> Result<VmValue, ExecutionError> {
        self.list_prepend_value(
            self.register(frame, list),
            self.register(frame, value),
            list_ownership,
            value_ownership,
        )
    }

    pub(super) fn list_remove(
        &mut self,
        frame: usize,
        list: Register,
        index: Register,
        list_ownership: ArgOwnership,
    ) -> Result<VmValue, ExecutionError> {
        self.list_remove_value(
            self.register(frame, list),
            self.register(frame, index),
            list_ownership,
        )
    }

    pub(super) fn replace_list_element(
        &mut self,
        frame: usize,
        list: Register,
        index: Register,
        value: Register,
        list_ownership: ArgOwnership,
        value_ownership: ArgOwnership,
    ) -> Result<VmValue, ExecutionError> {
        self.replace_list_element_value(
            self.register(frame, list),
            self.register(frame, index),
            self.register(frame, value),
            list_ownership,
            value_ownership,
        )
    }
}
