//! Helper functions for compiling various expression types.

use std::collections::HashMap;

use inkwell::values::BasicValueEnum;
use ori_ir::{DurationUnit, Name, SizeUnit};

use crate::builder::Builder;

impl<'a, 'll, 'tcx> Builder<'a, 'll, 'tcx> {
    /// Compile a config variable reference.
    /// Config variables are compile-time constants stored in locals.
    pub(crate) fn compile_config(
        &self,
        name: Name,
        locals: &HashMap<Name, BasicValueEnum<'ll>>,
    ) -> Option<BasicValueEnum<'ll>> {
        // Config variables should be pre-populated in locals by the caller
        locals.get(&name).copied()
    }

    /// Compile a function reference (@name).
    pub(crate) fn compile_function_ref(&self, name: Name) -> Option<BasicValueEnum<'ll>> {
        let fn_name = self.cx().interner.lookup(name);
        let func = self.cx().llmod().get_function(fn_name)?;
        Some(func.as_global_value().as_pointer_value().into())
    }

    /// Compile a duration literal.
    /// Durations are stored as i64 milliseconds.
    pub(crate) fn compile_duration(&self, value: u64, unit: DurationUnit) -> Option<BasicValueEnum<'ll>> {
        let millis = unit.to_millis(value);
        Some(self.cx().scx.type_i64().const_int(millis, false).into())
    }

    /// Compile a size literal.
    /// Sizes are stored as i64 bytes.
    pub(crate) fn compile_size(&self, value: u64, unit: SizeUnit) -> Option<BasicValueEnum<'ll>> {
        let bytes = unit.to_bytes(value);
        Some(self.cx().scx.type_i64().const_int(bytes, false).into())
    }
}
