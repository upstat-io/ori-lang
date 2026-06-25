//! Arena Range Types
//!
//! All range types for arena-allocated data. These are compact representations
//! that store start index and length, enabling efficient iteration over arena data.
//!
//! # Salsa Compatibility
//! All types have Copy, Clone, Eq, `PartialEq`, Hash, Debug for Salsa requirements.

use crate::macros::define_range;

define_range!(
    ParamRange,
    GenericParamRange,
    ArmRange,
    MapEntryRange,
    MapElementRange,
    FieldInitRange,
    StructLitFieldRange,
    NamedExprRange,
    CallArgRange,
    ListElementRange,
    TemplatePartRange,
    AccessStepRange,
);

#[cfg(test)]
mod tests;
