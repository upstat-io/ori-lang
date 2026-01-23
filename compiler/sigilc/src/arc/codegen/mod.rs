// Code Generation module for ARC Memory Management
//
// This module contains components for generating ARC code:
// - Scope tracking for destruction ordering
// - Retain/release insertion point identification
// - Elision analysis for optimization
// - Code emission for ARC operations

pub mod elision;
pub mod emitter;
pub mod insertion;
pub mod scope_tracker;

// Re-export main types
pub use elision::{can_apply_cow, ElisionAnalyzer, LivenessState, UseInfo};
pub use emitter::DefaultCodeEmitter;
pub use insertion::DefaultRefCountAnalyzer;
pub use scope_tracker::{LocalAllocation, ScopeInfo, ScopeKind, ScopeTracker};
