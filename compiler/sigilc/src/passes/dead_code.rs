// Dead code elimination pass for Sigil TIR
// Marks unreachable functions (not called from main or tests)

use super::{Pass, PassContext, PassError, PassResult};
use crate::ir::TModule;

/// Dead code elimination pass
/// Marks functions as unreachable if they are not called from main or any test
pub struct DeadCodePass;

impl Pass for DeadCodePass {
    fn name(&self) -> &'static str {
        "dead_code"
    }

    fn run(&self, ir: &mut TModule, ctx: &mut PassContext) -> Result<PassResult, PassError> {
        // Build call graph if not already built
        let call_graph = ctx.call_graph(ir).clone();

        // Count unreachable functions
        let mut removed_count = 0;

        // We don't actually remove functions (that could break things),
        // but we can mark them or report them for debugging
        for func in &ir.functions {
            if func.name != "main" && !call_graph.is_reachable(&func.name) {
                removed_count += 1;
                if ctx.debug.verbose {
                    eprintln!(
                        "[dead_code] Function '{}' is not reachable from main or tests",
                        func.name
                    );
                }
            }
        }

        if removed_count > 0 {
            Ok(PassResult::changed(removed_count))
        } else {
            Ok(PassResult::unchanged())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dead_code_pass_name() {
        let pass = DeadCodePass;
        assert_eq!(pass.name(), "dead_code");
    }

    #[test]
    fn test_dead_code_pass_not_required() {
        let pass = DeadCodePass;
        assert!(!pass.required());
    }
}
