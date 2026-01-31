//! Type problem rendering.
//!
//! Renders `TypeProblem` variants into user-facing Diagnostic messages.

use super::Render;
use crate::diagnostic::{Diagnostic, ErrorCode};
use crate::problem::TypeProblem;
use crate::suggest::suggest_similar;

impl Render for TypeProblem {
    fn render(&self) -> Diagnostic {
        match self {
            TypeProblem::TypeMismatch {
                span,
                expected,
                found,
            } => Diagnostic::error(ErrorCode::E2001)
                .with_message(format!(
                    "type mismatch: expected `{expected}`, found `{found}`"
                ))
                .with_label(*span, format!("expected `{expected}`")),

            TypeProblem::ArgCountMismatch {
                span,
                expected,
                found,
            } => {
                let plural = if *expected == 1 { "" } else { "s" };
                Diagnostic::error(ErrorCode::E2004)
                    .with_message(format!(
                        "wrong number of arguments: expected {expected}, found {found}"
                    ))
                    .with_label(*span, format!("expected {expected} argument{plural}"))
                    .with_suggestion(if *found > *expected {
                        "remove extra arguments"
                    } else {
                        "add missing arguments"
                    })
            }

            TypeProblem::TupleLengthMismatch {
                span,
                expected,
                found,
            } => Diagnostic::error(ErrorCode::E2001)
                .with_message(format!(
                    "tuple length mismatch: expected {expected}-tuple, found {found}-tuple"
                ))
                .with_label(*span, format!("expected {expected} elements")),

            TypeProblem::ListLengthMismatch {
                span,
                expected,
                found,
            } => Diagnostic::error(ErrorCode::E2001)
                .with_message(format!(
                    "list destructuring: expected at least {expected} elements, found {found}"
                ))
                .with_label(*span, format!("expected at least {expected} elements")),

            TypeProblem::InfiniteType { span } => Diagnostic::error(ErrorCode::E2008)
                .with_message("infinite type detected")
                .with_label(*span, "this would create an infinite type")
                .with_note("a type cannot contain itself"),

            TypeProblem::CannotInfer { span, context } => Diagnostic::error(ErrorCode::E2005)
                .with_message(format!("cannot infer type for {context}"))
                .with_label(*span, "type annotation needed")
                .with_suggestion("add explicit type annotation"),

            TypeProblem::UnknownType { span, name } => Diagnostic::error(ErrorCode::E2002)
                .with_message(format!("unknown type `{name}`"))
                .with_label(*span, format!("type `{name}` not found")),

            TypeProblem::NotCallable { span, found_type } => Diagnostic::error(ErrorCode::E2001)
                .with_message(format!("`{found_type}` is not callable"))
                .with_label(*span, "cannot call this as a function"),

            TypeProblem::NotIndexable { span, found_type } => Diagnostic::error(ErrorCode::E2001)
                .with_message(format!("`{found_type}` cannot be indexed"))
                .with_label(*span, "indexing not supported"),

            TypeProblem::NoSuchField {
                span,
                type_name,
                field_name,
                available_fields,
            } => {
                let mut diag = Diagnostic::error(ErrorCode::E2003)
                    .with_message(format!("no field `{field_name}` on type `{type_name}`"))
                    .with_label(
                        *span,
                        format!("`{field_name}` is not a field of `{type_name}`"),
                    );
                if !available_fields.is_empty() {
                    // Try to find a similar field name
                    if let Some(suggestion) =
                        suggest_similar(field_name, available_fields.iter().map(String::as_str))
                    {
                        diag = diag.with_suggestion(format!("try using `{suggestion}`"));
                    } else {
                        diag = diag.with_note(format!(
                            "available fields: {}",
                            available_fields.join(", ")
                        ));
                    }
                }
                diag
            }

            TypeProblem::NoSuchMethod {
                span,
                type_name,
                method_name,
                available_methods,
            } => {
                let mut diag = Diagnostic::error(ErrorCode::E2003)
                    .with_message(format!("no method `{method_name}` on type `{type_name}`"))
                    .with_label(*span, format!("`{method_name}` not found on `{type_name}`"));
                if !available_methods.is_empty() {
                    // Try to find a similar method name
                    if let Some(suggestion) =
                        suggest_similar(method_name, available_methods.iter().map(String::as_str))
                    {
                        diag = diag.with_suggestion(format!("try using `{suggestion}`"));
                    } else {
                        diag = diag.with_note(format!(
                            "available methods: {}",
                            available_methods.join(", ")
                        ));
                    }
                }
                diag
            }

            TypeProblem::InvalidBinaryOp {
                span,
                op,
                left_type,
                right_type,
            } => Diagnostic::error(ErrorCode::E2001)
                .with_message(format!(
                    "cannot apply `{op}` to `{left_type}` and `{right_type}`"
                ))
                .with_label(
                    *span,
                    format!("`{op}` cannot be applied to `{left_type}` and `{right_type}`"),
                ),

            TypeProblem::InvalidUnaryOp {
                span,
                op,
                operand_type,
            } => Diagnostic::error(ErrorCode::E2001)
                .with_message(format!("cannot apply `{op}` to `{operand_type}`"))
                .with_label(
                    *span,
                    format!("`{op}` cannot be applied to `{operand_type}`"),
                ),

            TypeProblem::MissingNamedArg { span, arg_name } => Diagnostic::error(ErrorCode::E2004)
                .with_message(format!("missing required argument `.{arg_name}:`"))
                .with_label(*span, format!("missing `.{arg_name}:`")),

            TypeProblem::UnknownNamedArg {
                span,
                arg_name,
                valid_args,
            } => {
                let mut diag = Diagnostic::error(ErrorCode::E2004)
                    .with_message(format!("unknown argument `.{arg_name}:`"))
                    .with_label(*span, format!("`.{arg_name}:` is not a valid argument"));
                if !valid_args.is_empty() {
                    diag = diag.with_note(format!("valid arguments: .{}", valid_args.join(", .")));
                }
                diag
            }

            TypeProblem::DuplicateNamedArg {
                span,
                arg_name,
                first_span,
            } => Diagnostic::error(ErrorCode::E2006)
                .with_message(format!("duplicate argument `.{arg_name}:`"))
                .with_label(*span, format!("`.{arg_name}:` provided more than once"))
                .with_secondary_label(*first_span, "first occurrence here"),

            TypeProblem::ReturnTypeMismatch {
                span,
                expected,
                found,
                func_name,
            } => Diagnostic::error(ErrorCode::E2001)
                .with_message(format!(
                    "return type mismatch in `{func_name}`: expected `{expected}`, found `{found}`"
                ))
                .with_label(*span, format!("expected `{expected}`")),

            TypeProblem::InvalidTryOperand { span, found_type } => {
                Diagnostic::error(ErrorCode::E2001)
                    .with_message(format!(
                        "`?` operator requires Result or Option, found `{found_type}`"
                    ))
                    .with_label(*span, "not Result or Option")
            }

            TypeProblem::InvalidAwait { span, found_type } => Diagnostic::error(ErrorCode::E2001)
                .with_message(format!(
                    "`await` requires async value, found `{found_type}`"
                ))
                .with_label(*span, "not async"),

            TypeProblem::ConditionNotBool { span, found_type } => {
                Diagnostic::error(ErrorCode::E2001)
                    .with_message(format!("condition must be `bool`, found `{found_type}`"))
                    .with_label(*span, "expected `bool`")
            }

            TypeProblem::NotIterable { span, found_type } => Diagnostic::error(ErrorCode::E2001)
                .with_message(format!("`{found_type}` is not iterable"))
                .with_label(*span, "cannot iterate over this"),

            TypeProblem::MatchArmTypeMismatch {
                span,
                first_type,
                this_type,
                first_span,
            } => Diagnostic::error(ErrorCode::E2001)
                .with_message(format!(
                    "match arms have incompatible types: `{first_type}` vs `{this_type}`"
                ))
                .with_label(*span, format!("expected `{first_type}`"))
                .with_secondary_label(*first_span, "first arm has this type"),

            TypeProblem::PatternTypeMismatch {
                span,
                expected,
                found,
            } => Diagnostic::error(ErrorCode::E2001)
                .with_message(format!(
                    "pattern type mismatch: expected `{expected}`, found `{found}`"
                ))
                .with_label(*span, format!("expected `{expected}`")),

            TypeProblem::CyclicType { span, type_name } => Diagnostic::error(ErrorCode::E2008)
                .with_message(format!("cyclic type definition for `{type_name}`"))
                .with_label(*span, "cycle detected here"),

            TypeProblem::ClosureSelfReference { span } => Diagnostic::error(ErrorCode::E2007)
                .with_message("closure cannot capture itself")
                .with_label(*span, "self-reference not allowed")
                .with_note("closures cannot recursively reference themselves"),
        }
    }
}
