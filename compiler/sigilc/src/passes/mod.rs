// Pass infrastructure for Sigil TIR
//
// Provides a framework for ordered transformations with dependencies.
// Passes can:
// - Transform the TIR (e.g., pattern lowering)
// - Optimize the TIR (e.g., constant folding)
// - Analyze the TIR (e.g., dead code detection)
//
// Module structure:
// - mod.rs: Pass trait and PassResult
// - manager.rs: PassManager for running passes
// - context.rs: PassContext for shared state
// - const_fold.rs: Constant folding optimization
// - dead_code.rs: Dead code elimination
// - pattern_lower.rs: Pattern -> loop lowering

mod const_fold;
mod context;
mod dead_code;
mod manager;
mod pattern_lower;
pub mod pipeline;
pub mod registry;

pub use const_fold::ConstantFoldingPass;
pub use context::{CallGraph, DebugConfig, PassContext};
pub use dead_code::DeadCodePass;
pub use manager::PassManager;
pub use pattern_lower::PatternLoweringPass;
pub use pipeline::PassPipeline;
pub use registry::{get_pass, has_pass, pass_names, PassInfo, PassRegistry};

use crate::ir::TModule;
use std::time::Duration;

/// Result of running a pass
#[derive(Debug, Clone)]
pub struct PassResult {
    /// Whether the pass made any changes
    pub changed: bool,
    /// Statistics about the pass execution
    pub stats: PassStats,
}

impl PassResult {
    pub fn unchanged() -> Self {
        PassResult {
            changed: false,
            stats: PassStats::default(),
        }
    }

    pub fn changed(items_transformed: usize) -> Self {
        PassResult {
            changed: true,
            stats: PassStats {
                duration: Duration::ZERO,
                items_transformed,
            },
        }
    }
}

/// Statistics collected during pass execution
#[derive(Debug, Clone, Default)]
pub struct PassStats {
    /// Time taken by the pass
    pub duration: Duration,
    /// Number of items transformed
    pub items_transformed: usize,
}

/// Error during pass execution
#[derive(Debug, Clone)]
pub struct PassError {
    pub pass_name: String,
    pub message: String,
}

impl PassError {
    pub fn new(pass_name: &str, message: impl Into<String>) -> Self {
        PassError {
            pass_name: pass_name.to_string(),
            message: message.into(),
        }
    }
}

impl std::fmt::Display for PassError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Pass '{}' failed: {}", self.pass_name, self.message)
    }
}

impl std::error::Error for PassError {}

/// Trait for compiler passes
pub trait Pass {
    /// Name of this pass (for debugging and logging)
    fn name(&self) -> &'static str;

    /// Whether this pass is required (cannot be disabled)
    fn required(&self) -> bool {
        false
    }

    /// Run the pass on the module
    fn run(&self, ir: &mut TModule, ctx: &mut PassContext) -> Result<PassResult, PassError>;

    /// Names of passes that must run before this one
    fn requires(&self) -> &[&'static str] {
        &[]
    }
}

/// Trait for passes that can be boxed
impl<T: Pass + ?Sized> Pass for Box<T> {
    fn name(&self) -> &'static str {
        (**self).name()
    }

    fn required(&self) -> bool {
        (**self).required()
    }

    fn run(&self, ir: &mut TModule, ctx: &mut PassContext) -> Result<PassResult, PassError> {
        (**self).run(ir, ctx)
    }

    fn requires(&self) -> &[&'static str] {
        (**self).requires()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pass_result_unchanged() {
        let result = PassResult::unchanged();
        assert!(!result.changed);
        assert_eq!(result.stats.items_transformed, 0);
    }

    #[test]
    fn test_pass_result_changed() {
        let result = PassResult::changed(5);
        assert!(result.changed);
        assert_eq!(result.stats.items_transformed, 5);
    }

    #[test]
    fn test_pass_error() {
        let err = PassError::new("test_pass", "something went wrong");
        assert_eq!(err.pass_name, "test_pass");
        assert!(err.to_string().contains("something went wrong"));
    }
}
