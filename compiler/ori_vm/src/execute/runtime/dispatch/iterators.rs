//! Iterator runtime-call adapters.

use ori_repr::executable::IteratorSource;

use crate::ExecutionError;

use super::super::super::operands::OperandAccess;
use super::super::super::value::VmValue;
use super::super::super::Interpreter;
use super::RuntimeSite;

impl Interpreter<'_> {
    pub(super) fn runtime_iter(
        &mut self,
        site: RuntimeSite,
        source: IteratorSource,
        operands: &mut impl OperandAccess,
    ) -> Result<VmValue, ExecutionError> {
        let [iterable] =
            self.runtime_values::<1>(site.frame, site.operands, site.call, operands)?;
        let ownership = self.runtime_argument_ownership(site.operands, 0, site.call)?;
        self.create_iterator_value(site.frame, site.destination, source, iterable, ownership)
    }

    pub(super) fn runtime_iter_next(
        &mut self,
        site: RuntimeSite,
        operands: &mut impl OperandAccess,
    ) -> Result<VmValue, ExecutionError> {
        let [iterator, _item_type] =
            self.runtime_values::<2>(site.frame, site.operands, site.call, operands)?;
        self.iter_next_value(iterator)
    }

    pub(super) fn runtime_iter_drop(
        &mut self,
        site: RuntimeSite,
        operands: &mut impl OperandAccess,
    ) -> Result<VmValue, ExecutionError> {
        let [iterator] =
            self.runtime_values::<1>(site.frame, site.operands, site.call, operands)?;
        self.iter_drop(iterator)
    }
}
