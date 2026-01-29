//! Subdiagnostic derive usage example.
//!
//! NOTE: This test is a documentation example. It cannot be compiled in
//! isolation because the macro generates code referencing `crate::diagnostic::Diagnostic`.

use ori_macros::Subdiagnostic;

#[derive(Subdiagnostic)]
#[label("expected type `{ty}`")]
pub struct ExpectedTypeLabel {
    #[primary_span]
    pub span: Span,
    pub ty: String,
}

#[derive(Subdiagnostic)]
#[note("the trait `{trait_name}` is not implemented for `{ty}`")]
pub struct TraitNotImplementedNote {
    #[primary_span]
    pub span: Span,
    pub trait_name: String,
    pub ty: String,
}

#[derive(Subdiagnostic)]
#[help("consider adding `#[derive({derive_name})]`")]
pub struct DeriveHelpSuggestion {
    #[primary_span]
    pub span: Span,
    pub derive_name: String,
}

fn main() {}
