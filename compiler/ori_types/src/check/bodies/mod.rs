//! Function body type checking passes.
//!
//! This module implements Passes 2-5 of module type checking (`typeck.md` CK-1):
//! - Pass 2: Function bodies (`functions::check_function_bodies`)
//! - Pass 3: Test bodies (`functions::check_test_bodies`)
//! - Pass 4: Impl method bodies (`impls::check_impl_bodies`)
//! - Pass 5: Def impl (default implementation) method bodies (`impls::check_def_impl_bodies`)
//!
//! # Architecture
//!
//! Each function body is checked in a child environment that:
//! 1. Inherits from the frozen base environment (contains all function signatures)
//! 2. Has parameter bindings added
//! 3. Has function scope context set (`current_function`, capabilities)
//!
//! ```text
//! Base Environment (frozen after Pass 1)
//!     │
//!     ├─ child for function foo
//!     │   ├─ param: x -> int
//!     │   └─ param: y -> str
//!     │
//!     └─ child for function bar
//!         └─ param: n -> int
//! ```
//!
//! # File layout
//!
//! `bodies/mod.rs` is a thin dispatch hub. Body-checking logic lives in the
//! submodule that owns each pass — `functions.rs` (Passes 2–3) and `impls.rs`
//! (Passes 4–5). Each public pass-dispatch function has exactly one canonical
//! home; `mod.rs` re-exports via `pub use` so `check::bodies::check_function_bodies`
//! and siblings continue to resolve without changing import paths.

pub mod functions;
pub mod impls;

pub use functions::{check_function_bodies, check_test_bodies};
pub use impls::{check_def_impl_bodies, check_impl_bodies};

#[cfg(test)]
mod tests;
