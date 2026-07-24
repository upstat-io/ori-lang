//! String and output runtime-call adapters.

use crate::ExecutionError;

use super::super::super::operands::OperandAccess;
use super::super::super::value::VmValue;
use super::super::super::Interpreter;
use super::RuntimeSite;

impl Interpreter<'_> {
    pub(super) fn runtime_to_string(
        &mut self,
        site: RuntimeSite,
        operands: &mut impl OperandAccess,
    ) -> Result<VmValue, ExecutionError> {
        let [value] = self.runtime_values::<1>(site.frame, site.operands, site.call, operands)?;
        self.convert_to_string(value)
    }

    pub(super) fn runtime_concat(
        &mut self,
        site: RuntimeSite,
        operands: &mut impl OperandAccess,
    ) -> Result<VmValue, ExecutionError> {
        let [left, right] =
            self.runtime_values::<2>(site.frame, site.operands, site.call, operands)?;
        self.concat(left, right)
    }

    pub(super) fn runtime_string_contains(
        &mut self,
        site: RuntimeSite,
        operands: &mut impl OperandAccess,
    ) -> Result<VmValue, ExecutionError> {
        let [value, needle] =
            self.runtime_values::<2>(site.frame, site.operands, site.call, operands)?;
        self.string_contains(value, needle)
    }

    pub(super) fn runtime_string_starts_with(
        &mut self,
        site: RuntimeSite,
        operands: &mut impl OperandAccess,
    ) -> Result<VmValue, ExecutionError> {
        let [value, prefix] =
            self.runtime_values::<2>(site.frame, site.operands, site.call, operands)?;
        self.string_starts_with(value, prefix)
    }

    pub(super) fn runtime_string_ends_with(
        &mut self,
        site: RuntimeSite,
        operands: &mut impl OperandAccess,
    ) -> Result<VmValue, ExecutionError> {
        let [value, suffix] =
            self.runtime_values::<2>(site.frame, site.operands, site.call, operands)?;
        self.string_ends_with(value, suffix)
    }

    pub(super) fn runtime_string_is_empty(
        &mut self,
        site: RuntimeSite,
        operands: &mut impl OperandAccess,
    ) -> Result<VmValue, ExecutionError> {
        let [value] = self.runtime_values::<1>(site.frame, site.operands, site.call, operands)?;
        self.string_is_empty(value)
    }

    pub(super) fn runtime_string_trim(
        &mut self,
        site: RuntimeSite,
        operands: &mut impl OperandAccess,
    ) -> Result<VmValue, ExecutionError> {
        let [value] = self.runtime_values::<1>(site.frame, site.operands, site.call, operands)?;
        self.string_trim(value)
    }

    pub(super) fn runtime_string_uppercase(
        &mut self,
        site: RuntimeSite,
        operands: &mut impl OperandAccess,
    ) -> Result<VmValue, ExecutionError> {
        let [value] = self.runtime_values::<1>(site.frame, site.operands, site.call, operands)?;
        self.string_uppercase(value)
    }

    pub(super) fn runtime_string_lowercase(
        &mut self,
        site: RuntimeSite,
        operands: &mut impl OperandAccess,
    ) -> Result<VmValue, ExecutionError> {
        let [value] = self.runtime_values::<1>(site.frame, site.operands, site.call, operands)?;
        self.string_lowercase(value)
    }

    pub(super) fn runtime_string_split(
        &mut self,
        site: RuntimeSite,
        operands: &mut impl OperandAccess,
    ) -> Result<VmValue, ExecutionError> {
        let [value, separator] =
            self.runtime_values::<2>(site.frame, site.operands, site.call, operands)?;
        self.string_split_value(value, separator)
    }

    pub(super) fn runtime_print(
        &mut self,
        site: RuntimeSite,
        operands: &mut impl OperandAccess,
    ) -> Result<VmValue, ExecutionError> {
        let [value] = self.runtime_values::<1>(site.frame, site.operands, site.call, operands)?;
        self.print(value)
    }

    pub(super) fn runtime_panic(
        &mut self,
        site: RuntimeSite,
        operands: &mut impl OperandAccess,
    ) -> Result<VmValue, ExecutionError> {
        let [message] = self.runtime_values::<1>(site.frame, site.operands, site.call, operands)?;
        self.panic(message)
    }
}
