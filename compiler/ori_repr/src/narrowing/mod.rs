//! Narrowing pipeline — integer and float narrowing passes.
//!
//! Narrows semantic types to smaller machine representations when the
//! compiler can prove no loss of precision or correctness:
//! - `int` (i64) → i8/i16/i32 via value range analysis
//! - `float` (f64) → f32 via precision analysis
//!
//! # Architecture
//!
//! The narrowing passes read from `ReprPlan` (ranges, field summaries)
//! and write back narrowed `MachineRepr` into the plan.
//!
//! Five modules:
//! - **`abi`**: ABI boundary classification and widening policy
//! - **`int`**: Integer struct/tuple field narrowing
//! - **`float`**: Float precision analysis and field narrowing
//! - **`overflow`** (future): Overflow guard insertion
//!
//! Narrowing phases:
//! - **Phase A**: Struct field narrowing via field-summary ranges/precision
//! - **Phase B**: Local variable narrowing + overflow guards
//! - **Phase C**: Collection element narrowing

pub mod abi;
pub mod float;
pub mod int;
pub mod overflow;

#[cfg(test)]
mod tests;
