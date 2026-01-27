//! Analysis passes for code generation.
//!
//! This module contains analysis passes that run before code generation
//! to determine optimizations like ARC elision.

mod ownership;

pub use ownership::{Ownership, OwnershipAnalysis, OwnershipInfo};
