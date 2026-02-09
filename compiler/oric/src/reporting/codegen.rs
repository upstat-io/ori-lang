//! Rendering for codegen problems.
//!
//! Delegates to [`CodegenProblem::into_diagnostic`] — codegen problems carry
//! all context needed for rendering without needing the string interner.

use crate::diagnostic::Diagnostic;
use crate::problem::CodegenProblem;
use crate::reporting::Render;
use ori_ir::StringInterner;

impl Render for CodegenProblem {
    fn render(&self, _interner: &StringInterner) -> Diagnostic {
        self.into_diagnostic()
    }
}
