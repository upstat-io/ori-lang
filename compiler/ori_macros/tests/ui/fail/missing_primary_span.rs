//! Error: Missing #[primary_span] attribute.

use ori_macros::Diagnostic;

#[derive(Diagnostic)]
#[diag(E1001, "some error")]
pub struct MissingPrimarySpan {
    pub span: Span,
}

fn main() {}
