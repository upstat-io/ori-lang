//! Diagnostic with suggestion example.
//!
//! NOTE: This test is a documentation example. It cannot be compiled in
//! isolation because the macro generates code referencing `crate::diagnostic::Diagnostic`.

use ori_macros::Diagnostic;

#[derive(Diagnostic)]
#[diag(E2001, "type mismatch: expected `{expected}`, found `{found}`")]
pub struct TypeMismatch {
    #[primary_span]
    #[label("expected `{expected}`")]
    pub span: Span,
    pub expected: String,
    pub found: String,
    #[suggestion("try converting with `int({found})`", code = "int({found})", applicability = "maybe-incorrect")]
    pub conversion_span: Option<Span>,
}

fn main() {}
