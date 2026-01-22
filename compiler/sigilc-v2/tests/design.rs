//! Design Tests for Sigil V2 Compiler
//!
//! **CRITICAL: Tests are the source of truth.**
//!
//! These tests validate that the compiler implementation conforms to the design
//! documented in `docs/compiler-design/v2/`. If a test fails, the code is wrong.
//!
//! ## Test Categories
//!
//! - `intern_*` - String and type interning invariants
//! - `arena_*` - Expression arena allocation contracts
//! - `parallel_*` - Parallelism architecture requirements
//! - `perf_*` - Performance budget assertions

#[path = "design/intern.rs"]
mod intern;

#[path = "design/arena.rs"]
mod arena;

#[path = "design/parallel.rs"]
mod parallel;
