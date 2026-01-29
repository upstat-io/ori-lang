//! Basic diagnostic derive usage example.
//!
//! NOTE: This test is a documentation example. It cannot be compiled in
//! isolation because the macro generates code referencing `crate::diagnostic::Diagnostic`.
//! Integration tests should be added to the `oric` crate.

// Required for macro to work:
// mod diagnostic {
//     pub struct Diagnostic { ... }
//     pub enum ErrorCode { E1001, ... }
//     pub struct Suggestion { ... }
//     pub enum Applicability { ... }
// }

use ori_macros::Diagnostic;

#[derive(Diagnostic)]
#[diag(E1001, "unexpected token: expected `{expected}`, found `{found}`")]
pub struct UnexpectedToken {
    #[primary_span]
    #[label("unexpected token here")]
    pub span: Span,
    pub expected: String,
    pub found: String,
}

fn main() {}
