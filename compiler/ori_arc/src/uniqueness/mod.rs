//! Uniqueness types for COW check elimination.
//!
//! Provides the [`Uniqueness`] lattice, [`CowMode`] annotation, and container
//! types ([`CowAnnotations`], [`DropHints`]) used by both the AIMS pipeline
//! and LLVM codegen.
//!
//! # Lattice
//!
//! The analysis operates over a three-point uniqueness lattice:
//!
//! ```text
//!          Unique
//!         /      \
//!    MaybeShared
//!         \      /
//!          Shared
//! ```
//!
//! - **Unique**: provably RC == 1. COW check can be eliminated.
//! - **`MaybeShared`**: unknown. Runtime check needed (conservative default).
//! - **Shared**: provably RC > 1. Slow path always taken.

mod annotations;
pub mod drop_hints;
mod lattice;

pub use annotations::CowAnnotations;
pub use drop_hints::DropHints;
pub use lattice::{CowMode, Uniqueness};
