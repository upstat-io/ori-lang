//! Type errors and diagnostics.

use crate::core::Type;
use ori_ir::{Name, Span, StringInterner};

/// Type error.
#[derive(Clone, Debug)]
pub enum TypeError {
    /// Type mismatch.
    TypeMismatch { expected: Type, found: Type },
    /// Argument count mismatch.
    ArgCountMismatch { expected: usize, found: usize },
    /// Tuple length mismatch.
    TupleLengthMismatch { expected: usize, found: usize },
    /// Infinite type (occurs check failure).
    InfiniteType,
    /// Unknown identifier.
    UnknownIdent(Name),
    /// Cannot infer type.
    CannotInfer,
}

impl TypeError {
    /// Convert to a diagnostic with helpful suggestions.
    pub fn to_diagnostic(
        &self,
        span: Span,
        interner: &StringInterner,
    ) -> ori_diagnostic::Diagnostic {
        use ori_diagnostic::{Diagnostic, ErrorCode};

        match self {
            TypeError::TypeMismatch { expected, found } => {
                let exp_str = expected.display(interner);
                let found_str = found.display(interner);

                let mut diag = Diagnostic::error(ErrorCode::E2001)
                    .with_message(format!(
                        "type mismatch: expected `{exp_str}`, found `{found_str}`",
                    ))
                    .with_label(span, format!("expected `{exp_str}`"));

                // Add helpful suggestions for common mistakes
                diag = match (expected, found) {
                    (Type::Bool, Type::Int) => diag.with_suggestion(
                        "use a comparison operator (e.g., `x != 0`) to convert int to bool",
                    ),
                    (Type::Int, Type::Float) => {
                        diag.with_suggestion("use `int(x)` to convert float to int")
                    }
                    (Type::Float, Type::Int) => {
                        diag.with_suggestion("use `float(x)` to convert int to float")
                    }
                    (Type::Str, _) => diag.with_suggestion("use `str(x)` to convert to string"),
                    (Type::List(_), t) if !matches!(t, Type::List(_)) => {
                        diag.with_suggestion("wrap the value in a list: `[x]`")
                    }
                    (Type::Option(_), t) if !matches!(t, Type::Option(_) | Type::Var(_)) => {
                        diag.with_suggestion("wrap the value in Some: `Some(x)`")
                    }
                    _ => diag,
                };

                diag
            }
            TypeError::ArgCountMismatch { expected, found } => {
                let plural = if *expected == 1 { "" } else { "s" };
                Diagnostic::error(ErrorCode::E2004)
                    .with_message(format!(
                        "wrong number of arguments: expected {expected}, found {found}",
                    ))
                    .with_label(span, format!("expected {expected} argument{plural}"))
                    .with_suggestion(if *found > *expected {
                        "remove extra arguments"
                    } else {
                        "add missing arguments"
                    })
            }
            TypeError::TupleLengthMismatch { expected, found } => {
                Diagnostic::error(ErrorCode::E2001)
                    .with_message(format!(
                        "tuple length mismatch: expected {expected}-tuple, found {found}-tuple",
                    ))
                    .with_label(span, format!("expected {expected} elements"))
            }
            TypeError::InfiniteType => Diagnostic::error(ErrorCode::E2005)
                .with_message("cannot construct infinite type (occurs check failed)")
                .with_label(span, "this creates a self-referential type")
                .with_suggestion("break the cycle by introducing an intermediate type"),
            TypeError::UnknownIdent(name) => {
                let name_str = interner.lookup(*name);
                Diagnostic::error(ErrorCode::E2003)
                    .with_message(format!("unknown identifier `{name_str}`"))
                    .with_label(span, "not found in this scope")
                    .with_suggestion(format!(
                        "check spelling, or add a definition for `{name_str}`"
                    ))
            }
            TypeError::CannotInfer => Diagnostic::error(ErrorCode::E2005)
                .with_message("cannot infer type: insufficient context")
                .with_label(span, "type annotation needed here")
                .with_suggestion("add an explicit type annotation like `: int` or `: str`"),
        }
    }
}
