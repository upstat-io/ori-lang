//! Uniqueness types for COW check elimination.
//!
//! Provides the [`Uniqueness`] lattice and the frozen [`CowMode`],
//! [`CowAnnotations`], and [`DropHints`] facts produced by AIMS. VM, LLVM,
//! native, compiled-WASM, and JIT projections consume these facts without
//! re-deriving ownership policy. The facts do not choose a backend's ABI,
//! field offsets, register layout, or VM slot layout.
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
//! - **Unique**: exactly one live logical owner. A physical sharing observation can be eliminated.
//! - **`MaybeShared`**: logical sharing is unknown. A physical observation or conservative copy is needed.
//! - **Shared**: multiple logical owners or a proven sharing obligation. The copy path is required.

mod annotations;
pub mod drop_hints;
mod lattice;

pub use annotations::CowAnnotations;
pub use drop_hints::DropHints;
pub use lattice::{CowMode, Uniqueness};
