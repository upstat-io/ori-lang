//! Parse problem rendering.
//!
//! Delegates to `ParseProblem::into_diagnostic()` for rendering.

use super::Render;
use crate::diagnostic::Diagnostic;
use crate::problem::ParseProblem;
use ori_ir::StringInterner;

impl Render for ParseProblem {
    fn render(&self, interner: &StringInterner) -> Diagnostic {
        self.into_diagnostic(interner)
    }
}
