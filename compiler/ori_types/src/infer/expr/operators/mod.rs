//! Operator inference — binary, unary, cast, and assignment operators.

mod assignment;
mod binary;
mod conversion;
mod dispatch;
mod unary;

pub(crate) use assignment::infer_assign;
pub(crate) use binary::infer_binary;
pub(crate) use conversion::infer_cast;
#[cfg(test)]
pub(crate) use dispatch::has_comparable_trait;
pub(crate) use unary::infer_unary;
