//! Collection runtime-call adapters.

use crate::ExecutionError;

use super::super::super::operands::OperandAccess;
use super::super::super::value::VmValue;
use super::super::super::Interpreter;
use super::super::list_mutations::ListMutationCall;
use super::RuntimeSite;

impl Interpreter<'_> {
    pub(super) fn runtime_list_new(
        &mut self,
        site: RuntimeSite,
        operands: &mut impl OperandAccess,
    ) -> Result<VmValue, ExecutionError> {
        let [capacity, _element_type] =
            self.runtime_values::<2>(site.frame, site.operands, site.call, operands)?;
        self.list_new(capacity)
    }

    pub(super) fn runtime_list_free(
        &mut self,
        site: RuntimeSite,
        operands: &mut impl OperandAccess,
    ) -> Result<VmValue, ExecutionError> {
        let [builder, _element_type] =
            self.runtime_values::<2>(site.frame, site.operands, site.call, operands)?;
        self.list_free(builder)
    }

    pub(super) fn runtime_list_builder_push(
        &mut self,
        site: RuntimeSite,
        operands: &mut impl OperandAccess,
    ) -> Result<VmValue, ExecutionError> {
        let [builder, value, _element_type] =
            self.runtime_values::<3>(site.frame, site.operands, site.call, operands)?;
        self.list_builder_push(builder, value)
    }

    pub(super) fn runtime_list_push(
        &mut self,
        site: RuntimeSite,
        operands: &mut impl OperandAccess,
    ) -> Result<VmValue, ExecutionError> {
        self.execute_list_mutation(site.frame, ListMutationCall::Push, site.operands, operands)
    }

    pub(super) fn runtime_list_insert(
        &mut self,
        site: RuntimeSite,
        operands: &mut impl OperandAccess,
    ) -> Result<VmValue, ExecutionError> {
        self.execute_list_mutation(
            site.frame,
            ListMutationCall::Insert,
            site.operands,
            operands,
        )
    }

    pub(super) fn runtime_list_remove(
        &mut self,
        site: RuntimeSite,
        operands: &mut impl OperandAccess,
    ) -> Result<VmValue, ExecutionError> {
        self.execute_list_mutation(
            site.frame,
            ListMutationCall::Remove,
            site.operands,
            operands,
        )
    }

    pub(super) fn runtime_list_prepend(
        &mut self,
        site: RuntimeSite,
        operands: &mut impl OperandAccess,
    ) -> Result<VmValue, ExecutionError> {
        self.execute_list_mutation(
            site.frame,
            ListMutationCall::Prepend,
            site.operands,
            operands,
        )
    }

    pub(super) fn runtime_list_take(
        &mut self,
        site: RuntimeSite,
        operands: &mut impl OperandAccess,
    ) -> Result<VmValue, ExecutionError> {
        let [builder] = self.runtime_values::<1>(site.frame, site.operands, site.call, operands)?;
        self.list_take(builder)
    }

    pub(super) fn runtime_index(
        &mut self,
        site: RuntimeSite,
        operands: &mut impl OperandAccess,
    ) -> Result<VmValue, ExecutionError> {
        let [collection, index] =
            self.runtime_values::<2>(site.frame, site.operands, site.call, operands)?;
        self.index(collection, index)
    }

    pub(super) fn runtime_list_set(
        &mut self,
        site: RuntimeSite,
        operands: &mut impl OperandAccess,
    ) -> Result<VmValue, ExecutionError> {
        self.execute_list_mutation(site.frame, ListMutationCall::Set, site.operands, operands)
    }

    pub(super) fn runtime_length(
        &mut self,
        site: RuntimeSite,
        operands: &mut impl OperandAccess,
    ) -> Result<VmValue, ExecutionError> {
        let [value] = self.runtime_values::<1>(site.frame, site.operands, site.call, operands)?;
        self.length_value(value)
    }

    pub(super) fn runtime_range_length(
        &mut self,
        site: RuntimeSite,
        operands: &mut impl OperandAccess,
    ) -> Result<VmValue, ExecutionError> {
        let [value] = self.runtime_values::<1>(site.frame, site.operands, site.call, operands)?;
        self.range_length_value(value)
    }
}
