// Type checker diagnostics for Sigil
//
// Provides structured error constructors for common type checking errors.
// This module supports gradual migration from Result<T, String> to DiagnosticResult<T>.

use crate::ast::TypeExpr;
use crate::errors::codes::ErrorCode;
use crate::errors::{Diagnostic, DiagnosticResult, Span};

/// Create a type mismatch diagnostic with detailed information
pub fn type_mismatch_diagnostic(
    expected: &TypeExpr,
    found: &TypeExpr,
    span: Span,
    context: Option<&str>,
) -> Diagnostic {
    let mut diag = Diagnostic::error(
        ErrorCode::E3001,
        format!(
            "type mismatch: expected {}, found {}",
            format_type(expected),
            format_type(found)
        ),
    )
    .with_label(span, format!("expected {}", format_type(expected)));

    if let Some(ctx) = context {
        diag = diag.with_note(ctx.to_string());
    }

    diag
}

/// Create an unknown identifier diagnostic
pub fn unknown_ident_diagnostic(name: &str, span: Span) -> Diagnostic {
    Diagnostic::error(ErrorCode::E3002, format!("unknown identifier '{}'", name))
        .with_label(span, "not found in this scope")
}

/// Create an unknown type diagnostic
pub fn unknown_type_diagnostic(name: &str, span: Span) -> Diagnostic {
    Diagnostic::error(ErrorCode::E3003, format!("unknown type '{}'", name))
        .with_label(span, "type not found")
}

/// Create a wrong number of arguments diagnostic
pub fn wrong_arg_count_diagnostic(
    func_name: &str,
    expected: usize,
    found: usize,
    span: Span,
) -> Diagnostic {
    Diagnostic::error(
        ErrorCode::E3004,
        format!(
            "function '{}' takes {} argument{}, but {} {} given",
            func_name,
            expected,
            if expected == 1 { "" } else { "s" },
            found,
            if found == 1 { "was" } else { "were" }
        ),
    )
    .with_label(
        span,
        format!(
            "expected {} argument{}",
            expected,
            if expected == 1 { "" } else { "s" }
        ),
    )
}

/// Create a cannot infer type diagnostic
pub fn cannot_infer_diagnostic(context: &str, span: Span) -> Diagnostic {
    Diagnostic::error(
        ErrorCode::E3005,
        format!("cannot infer type for {}", context),
    )
    .with_label(span, "type annotation needed")
    .with_help("consider adding an explicit type annotation")
}

/// Create an invalid operation diagnostic
pub fn invalid_operation_diagnostic(op: &str, type_name: &str, span: Span) -> Diagnostic {
    Diagnostic::error(
        ErrorCode::E3006,
        format!("cannot apply {} to {}", op, type_name),
    )
    .with_label(span, format!("invalid for type {}", type_name))
}

/// Create a missing test diagnostic
pub fn missing_test_diagnostic(func_name: &str, span: Span) -> Diagnostic {
    Diagnostic::error(
        ErrorCode::E3007,
        format!("function '{}' has no tests", func_name),
    )
    .with_label(span, "no test found")
    .with_help(format!(
        "add a test: @test_name tests @{} () -> void = run(...)",
        func_name
    ))
}

/// Create an unknown method diagnostic
pub fn unknown_method_diagnostic(method: &str, receiver_type: &TypeExpr, span: Span) -> Diagnostic {
    Diagnostic::error(
        ErrorCode::E3008,
        format!(
            "no method '{}' found for type {}",
            method,
            format_type(receiver_type)
        ),
    )
    .with_label(span, "method not found")
}

/// Create an invalid pattern diagnostic
pub fn invalid_pattern_diagnostic(pattern: &str, reason: &str, span: Span) -> Diagnostic {
    Diagnostic::error(
        ErrorCode::E3009,
        format!("invalid use of pattern '{}'", pattern),
    )
    .with_label(span, reason)
}

/// Create a duplicate definition diagnostic
pub fn duplicate_def_diagnostic(
    name: &str,
    kind: &str,
    span: Span,
    original_span: Option<Span>,
) -> Diagnostic {
    let mut diag = Diagnostic::error(
        ErrorCode::E3010,
        format!("{} '{}' is defined multiple times", kind, name),
    )
    .with_label(span, "duplicate definition");

    if let Some(orig) = original_span {
        diag = diag.with_secondary_label(orig, "first defined here");
    }

    diag
}

/// Create an immutable reassignment diagnostic
pub fn immutable_reassign_diagnostic(name: &str, span: Span) -> Diagnostic {
    Diagnostic::error(
        ErrorCode::E3001,
        format!("cannot assign twice to immutable variable '{}'", name),
    )
    .with_label(span, "cannot assign to immutable variable")
    .with_help(format!(
        "consider making this binding mutable: `let mut {}`",
        name
    ))
}

/// Create an undeclared variable diagnostic
pub fn undeclared_var_diagnostic(name: &str, span: Span) -> Diagnostic {
    Diagnostic::error(
        ErrorCode::E3002,
        format!("cannot assign to undeclared variable '{}'", name),
    )
    .with_label(span, "variable not declared")
    .with_help(format!(
        "use `let {} = ...` to declare the variable first",
        name
    ))
}

/// Create a non-iterable type diagnostic
pub fn non_iterable_diagnostic(type_expr: &TypeExpr, span: Span) -> Diagnostic {
    Diagnostic::error(
        ErrorCode::E3006,
        format!("cannot iterate over {}", format_type(type_expr)),
    )
    .with_label(span, "not iterable")
    .with_note("only lists and ranges can be iterated")
}

// ============================================================================
// String error format helpers (for gradual migration)
// ============================================================================

/// Format a type expression for display
pub fn format_type(ty: &TypeExpr) -> String {
    match ty {
        TypeExpr::Named(name) => name.clone(),
        TypeExpr::Generic(name, params) => {
            if params.is_empty() {
                name.clone()
            } else {
                format!(
                    "{}<{}>",
                    name,
                    params
                        .iter()
                        .map(format_type)
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            }
        }
        TypeExpr::List(inner) => format!("[{}]", format_type(inner)),
        TypeExpr::Map(key, value) => format!("Map<{}, {}>", format_type(key), format_type(value)),
        TypeExpr::Tuple(types) => {
            format!(
                "({})",
                types.iter().map(format_type).collect::<Vec<_>>().join(", ")
            )
        }
        TypeExpr::Function(input, output) => {
            format!("{} -> {}", format_type(input), format_type(output))
        }
        TypeExpr::Optional(inner) => format!("?{}", format_type(inner)),
        TypeExpr::Record(fields) => {
            let field_strs: Vec<String> = fields
                .iter()
                .map(|(name, ty)| format!("{}: {}", name, format_type(ty)))
                .collect();
            format!("{{ {} }}", field_strs.join(", "))
        }
        TypeExpr::DynTrait(trait_name) => format!("dyn {}", trait_name),
    }
}

/// Convert string error to diagnostic (for gradual migration)
/// Uses a default span when actual span is not available
pub fn string_to_diagnostic(msg: String) -> Diagnostic {
    Diagnostic::error(ErrorCode::E0000, msg)
}

/// Extension trait for Result<T, String> to add diagnostic conversion methods
pub trait TypeResultExt<T> {
    /// Convert a string error to a diagnostic with a span
    fn with_span(self, span: Span) -> DiagnosticResult<T>;

    /// Convert with default span (for when span info isn't available)
    fn into_diag(self) -> DiagnosticResult<T>;
}

impl<T> TypeResultExt<T> for Result<T, String> {
    fn with_span(self, span: Span) -> DiagnosticResult<T> {
        self.map_err(|msg| {
            Diagnostic::error(ErrorCode::E0000, msg).with_label(span, "error occurred here")
        })
    }

    fn into_diag(self) -> DiagnosticResult<T> {
        self.map_err(|msg| Diagnostic::error(ErrorCode::E0000, msg))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_type_named() {
        let ty = TypeExpr::Named("int".to_string());
        assert_eq!(format_type(&ty), "int");
    }

    #[test]
    fn test_format_type_list() {
        let ty = TypeExpr::List(Box::new(TypeExpr::Named("int".to_string())));
        assert_eq!(format_type(&ty), "[int]");
    }

    #[test]
    fn test_format_type_function() {
        let ty = TypeExpr::Function(
            Box::new(TypeExpr::Named("int".to_string())),
            Box::new(TypeExpr::Named("bool".to_string())),
        );
        assert_eq!(format_type(&ty), "int -> bool");
    }

    #[test]
    fn test_type_mismatch_diagnostic() {
        let expected = TypeExpr::Named("int".to_string());
        let found = TypeExpr::Named("str".to_string());
        let span = Span::new("test.si", 10..15);

        let diag = type_mismatch_diagnostic(&expected, &found, span, None);
        assert_eq!(diag.code, ErrorCode::E3001);
        assert!(diag.message.contains("int"));
        assert!(diag.message.contains("str"));
    }

    #[test]
    fn test_unknown_ident_diagnostic() {
        let span = Span::new("test.si", 5..10);
        let diag = unknown_ident_diagnostic("foo", span);
        assert_eq!(diag.code, ErrorCode::E3002);
        assert!(diag.message.contains("foo"));
    }

    #[test]
    fn test_wrong_arg_count_diagnostic() {
        let span = Span::new("test.si", 0..10);
        let diag = wrong_arg_count_diagnostic("myFunc", 2, 3, span);
        assert_eq!(diag.code, ErrorCode::E3004);
        assert!(diag.message.contains("2"));
        assert!(diag.message.contains("3"));
    }

    #[test]
    fn test_immutable_reassign_diagnostic() {
        let span = Span::new("test.si", 0..5);
        let diag = immutable_reassign_diagnostic("x", span);
        assert!(diag.help.iter().any(|h| h.contains("let mut")));
    }

    #[test]
    fn test_type_result_ext() {
        let ok_result: Result<i32, String> = Ok(42);
        let diag_result = ok_result.into_diag();
        assert!(diag_result.is_ok());

        let err_result: Result<i32, String> = Err("some error".to_string());
        let diag_result = err_result.into_diag();
        assert!(diag_result.is_err());
        let diag = diag_result.unwrap_err();
        assert!(diag.message.contains("some error"));
    }
}
