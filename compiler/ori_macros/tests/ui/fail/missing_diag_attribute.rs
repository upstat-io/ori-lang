//! Error: Missing #[diag(...)] attribute.

use ori_macros::Diagnostic;

#[derive(Diagnostic)]
pub struct MissingDiag {
    #[primary_span]
    pub span: Span,
}

fn main() {}
