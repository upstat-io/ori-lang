//! Lex problem rendering.
//!
//! Delegates to `LexProblem::into_diagnostic()` for rendering.

use super::Render;
use crate::diagnostic::Diagnostic;
use crate::problem::LexProblem;
use ori_ir::StringInterner;

impl Render for LexProblem {
    fn render(&self, interner: &StringInterner) -> Diagnostic {
        self.into_diagnostic(interner)
    }
}
