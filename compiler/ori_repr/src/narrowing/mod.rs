//! Narrowing pipeline (§04 integer, §05 float).
//!
//! Narrows semantic types to smaller machine representations when the
//! compiler can prove no loss of precision or correctness:
//! - `int` (i64) → i8/i16/i32 via value range analysis (§04)
//! - `float` (f64) → f32 via precision analysis (§05)
//!
//! # Architecture
//!
//! The narrowing passes read from `ReprPlan` (ranges, field summaries)
//! and write back narrowed `MachineRepr` into the plan.
//!
//! Five modules:
//! - **`abi`** (§04.2): ABI boundary classification and widening policy
//! - **`int`** (§04.1): Integer struct/tuple field narrowing
//! - **`float`** (§05.1): Float precision analysis and field narrowing
//! - **`overflow`** (§04.3, future): Overflow guard insertion
//!
//! Narrowing phases:
//! - **Phase A** (§04.1, §05.1): Struct field narrowing via field-summary ranges/precision
//! - **Phase B** (§04.2–§04.3): Local variable narrowing + overflow guards
//! - **Phase C** (§04.4): Collection element narrowing

pub mod abi;
pub mod float;
pub mod int;
pub mod overflow;

#[cfg(test)]
mod tests;
