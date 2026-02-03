//! Semantic problem rendering.
//!
//! Delegates to `SemanticProblem::into_diagnostic()` for rendering.

use super::Render;
use crate::diagnostic::Diagnostic;
use crate::problem::SemanticProblem;
use ori_ir::StringInterner;

impl Render for SemanticProblem {
    fn render(&self, interner: &StringInterner) -> Diagnostic {
        self.into_diagnostic(interner)
    }
}
