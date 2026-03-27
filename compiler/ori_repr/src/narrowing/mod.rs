//! Integer narrowing pipeline (§04).
//!
//! Narrows `int` (semantic i64) to the smallest machine integer (i8/i16/i32)
//! that preserves correctness, based on value ranges computed by §03.
//!
//! # Architecture
//!
//! The narrowing pass reads ranges from `ReprPlan::field_range()` and
//! `ReprPlan::var_range()`, applies conservatism rules, and writes back
//! narrowed `MachineRepr` into the plan.
//!
//! Three phases:
//! - **Phase A** (§04.1): Struct/tuple field narrowing via field-summary ranges
//! - **Phase B** (§04.2–§04.3): Local variable narrowing + overflow guards
//! - **Phase C** (§04.4): Collection element narrowing

pub mod int;

#[cfg(test)]
mod tests;
