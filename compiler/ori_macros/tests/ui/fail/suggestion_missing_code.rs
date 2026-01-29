//! Error: Suggestion attribute missing code parameter.

use ori_macros::Diagnostic;

#[derive(Diagnostic)]
#[diag(E1001, "some error")]
pub struct SuggestionMissingCode {
    #[primary_span]
    pub span: Span,
    #[suggestion("try this")]
    pub suggestion_span: Span,
}

fn main() {}
