//! Derived method evaluation for `@derive(...)` attributes.
//!
//! Uses [`DeriveStrategy`](ori_ir::derives::strategy::DeriveStrategy) from `ori_ir`
//! to drive field iteration and result combination. Each strategy variant
//! (`ForEachField`, `FormatFields`, etc.) has a corresponding handler that
//! interprets the strategy using `Value` operations.
//!
//! - [`for_each_field`]: Eq / Compare / Hash field-walking strategy
//! - [`format_fields`]: Debug / Printable formatting strategy
//! - [`default_construct`]: Default-value construction strategy

mod default_construct;
mod for_each_field;
mod format_fields;

use ori_ir::{DerivedMethodInfo, StructBody};

use super::Interpreter;
use crate::{EvalResult, Value};

impl Interpreter<'_> {
    /// Evaluate a derived method using its [`DeriveStrategy`].
    ///
    /// Dispatches on the strategy's `struct_body` to select the appropriate
    /// evaluation handler: field comparison, string formatting, cloning, or
    /// default construction.
    pub(super) fn eval_derived_method(
        &mut self,
        receiver: Value,
        info: &DerivedMethodInfo,
        args: &[Value],
    ) -> EvalResult {
        let strategy = info.trait_kind.strategy();
        match strategy.struct_body {
            StructBody::ForEachField { field_op, combine } => {
                self.eval_for_each_field(receiver, info, args, field_op, combine)
            }
            StructBody::FormatFields {
                open,
                separator,
                suffix,
                include_names,
            } => self.eval_format_fields(receiver, info, open, separator, suffix, include_names),
            StructBody::CloneFields => Ok(receiver),
            StructBody::DefaultConstruct => self.eval_default_construct(receiver, info),
        }
    }
}
