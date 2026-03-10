//! Expression IDs and ranges for flat AST.
//!
//! Per design spec A-data-structuresmd:
//! - `ExprId(u32)` instead of `Box<Expr>` for 50% memory savings
//! - `ExprRange` for argument lists (6 bytes vs 24+ for Vec)
//! - All Salsa-required traits

mod expr;
mod function;
mod parsed_type;
mod pattern;
mod stmt;

pub use expr::{ExprId, ExprRange};
pub use function::{FunctionExpId, FunctionSeqId};
pub use parsed_type::{ParsedTypeId, ParsedTypeRange};
pub use pattern::{BindingPatternId, MatchPatternId, MatchPatternRange};
pub use stmt::{StmtId, StmtRange};

#[cfg(test)]
mod tests;
