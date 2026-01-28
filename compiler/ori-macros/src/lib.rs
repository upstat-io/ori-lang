//! Procedural macros for the Ori compiler.
//!
//! This crate provides derive macros for diagnostics, enabling
//! declarative error definitions instead of manual `Render` implementations.
//!
//! # Diagnostic Derive
//!
//! The `Diagnostic` derive macro generates diagnostic creation code:
//!
//! ```text
//! #[derive(Diagnostic)]
//! #[diag(E2001, "type mismatch: expected `{expected}`, found `{found}`")]
//! pub struct TypeMismatch {
//!     #[primary_span]
//!     #[label("expected `{expected}`")]
//!     pub span: Span,
//!     pub expected: String,
//!     pub found: String,
//!     #[suggestion("convert with `int({name})`", code = "int({name})", applicability = "maybe-incorrect")]
//!     pub conversion_span: Option<Span>,
//! }
//! ```
//!
//! This generates an `IntoDiagnostic` implementation that creates a `Diagnostic`
//! with the specified error code, message, labels, and suggestions.

mod diagnostic;
mod subdiagnostic;

use proc_macro::TokenStream;

/// Derive macro for creating diagnostics from structs.
///
/// # Attributes
///
/// ## Struct-level
/// - `#[diag(CODE, "message")]` - Required. Specifies error code and message template.
///
/// ## Field-level
/// - `#[primary_span]` - Mark this span as the primary error location.
/// - `#[label("message")]` - Add a label to this span.
/// - `#[note("message")]` - Add a note using this field's value.
/// - `#[suggestion("msg", code = "...", applicability = "...")]` - Add a suggestion.
///
/// # Applicability Values
/// - `"machine-applicable"` - Safe to auto-apply
/// - `"maybe-incorrect"` - Might be wrong
/// - `"has-placeholders"` - Contains placeholders
/// - `"unspecified"` - Unknown confidence
///
/// # Example
///
/// ```text
/// #[derive(Diagnostic)]
/// #[diag(E1001, "unexpected token")]
/// pub struct UnexpectedToken {
///     #[primary_span]
///     #[label("unexpected token here")]
///     pub span: Span,
///     pub expected: String,
///     pub found: String,
/// }
///
/// // Usage:
/// let err = UnexpectedToken {
///     span: token.span,
///     expected: "identifier".to_string(),
///     found: "number".to_string(),
/// };
/// let diagnostic = err.into_diagnostic();
/// ```
#[proc_macro_derive(
    Diagnostic,
    attributes(diag, primary_span, label, note, suggestion, help)
)]
pub fn derive_diagnostic(input: TokenStream) -> TokenStream {
    diagnostic::derive_diagnostic(input)
}

/// Derive macro for subdiagnostics (additional labels, notes, suggestions).
///
/// Subdiagnostics can be added to an existing Diagnostic using the
/// `AddToDiagnostic` trait.
///
/// # Example
///
/// ```text
/// #[derive(Subdiagnostic)]
/// #[label("this type was expected")]
/// pub struct ExpectedTypeLabel {
///     #[primary_span]
///     pub span: Span,
///     pub ty: String,
/// }
///
/// // Usage:
/// diagnostic.add_subdiagnostic(ExpectedTypeLabel { span, ty });
/// ```
#[proc_macro_derive(Subdiagnostic, attributes(label, note, suggestion, help, primary_span))]
pub fn derive_subdiagnostic(input: TokenStream) -> TokenStream {
    subdiagnostic::derive_subdiagnostic(input)
}
