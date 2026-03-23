//! Fat pointer iteration tests.
//!
//! Tests for iterating over collections whose elements require Drop
//! (str, [T], closures, structs with Drop fields). These tests verify
//! that element-level RC cleanup happens correctly — no leaks, no
//! double-frees.
//!
//! Organized by test category:
//! - `str_list`: T1 — `[str]` basic iteration patterns
//! - `function_param`: F5 — borrowed parameter iteration across types
//! - `nested_list`: T2/T3 — `[[int]]`/`[[str]]` nested iteration
//! - `map_set`: T8/T9 — map/set iteration with string keys/values
//! - `struct_sum_tuple`: T4-T7 — struct, Option, Result, tuple elements
//! - `for_yield`: F3/F10 — yield transform and identity, break/continue
//! - `cow`: F11 — COW mutation (push element during iteration)
//! - `unwind`: panic/unwind during iteration with catch recovery
//! - `generalize`: `elem_dec_fn` generalization across all Drop types
//! - `control_flow`: cross-type regression matrix (type × pattern)

#![expect(
    clippy::needless_raw_string_hashes,
    reason = "readability in test program literals"
)]

mod control_flow;
mod conversion;
mod cow;
mod for_yield;
mod function_param;
mod generalize;
mod map_set;
mod method_collect;
mod nested_list;
mod str_list;
mod struct_sum_tuple;
mod unwind;
