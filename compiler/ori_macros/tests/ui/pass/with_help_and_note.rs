//! Diagnostic with help and note example.
//!
//! NOTE: This test is a documentation example. It cannot be compiled in
//! isolation because the macro generates code referencing `crate::diagnostic::Diagnostic`.

use ori_macros::Diagnostic;

#[derive(Diagnostic)]
#[diag(E3001, "undefined variable `{name}`")]
pub struct UndefinedVariable {
    #[primary_span]
    #[label("not found in this scope")]
    pub span: Span,
    pub name: String,
    #[note("variables must be declared before use")]
    pub note_field: (),
    #[help("did you mean `{suggestion}`?")]
    pub suggestion: String,
}

fn main() {}
