//! Parse error types.

use ori_ir::Span;

/// Parse error with error code for rich diagnostics.
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct ParseError {
    /// Error code for searchability.
    pub code: ori_diagnostic::ErrorCode,
    /// Human-readable message.
    pub message: String,
    /// Location of the error.
    pub span: Span,
    /// Optional context for suggestions.
    pub context: Option<String>,
}

impl ParseError {
    /// Create a new parse error.
    pub fn new(code: ori_diagnostic::ErrorCode, message: impl Into<String>, span: Span) -> Self {
        ParseError {
            code,
            message: message.into(),
            span,
            context: None,
        }
    }

    /// Add context for better error messages.
    #[must_use]
    pub fn with_context(mut self, context: impl Into<String>) -> Self {
        self.context = Some(context.into());
        self
    }

    /// Convert to a full Diagnostic for rich error reporting.
    pub fn to_diagnostic(&self) -> ori_diagnostic::Diagnostic {
        ori_diagnostic::Diagnostic::error(self.code)
            .with_message(&self.message)
            .with_label(self.span, self.context.as_deref().unwrap_or("here"))
    }
}
