//! Type checker phase tests.
//!
//! Tests for the `ori_typeck` and `ori_types` crates, validating:
//! - Type inference scenarios
//! - Unification edge cases
//! - Trait resolution
//! - Generic instantiation
//! - Error message quality
//!
//! # Test Organization
//!
//! - `types` - Core type system tests (Type enum, unification, schemes)
//! - `type_interner` - Type interning tests (primitives, roundtrips, sharing)

mod type_interner;
mod types;
