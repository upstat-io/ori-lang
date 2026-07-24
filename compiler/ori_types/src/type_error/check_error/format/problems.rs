//! Type-problem detail formatting shared by simple and rich renderers.

use crate::type_error::TypeProblem;
use crate::Idx;

/// Generate a rich problem message using a type formatter.
///
/// Uses the provided formatter for full type names with backtick wrapping,
/// instead of `Idx::display_name()`.
pub(super) fn problem_message_rich(
    problem: &TypeProblem,
    format_type: &dyn Fn(Idx) -> String,
) -> Option<String> {
    problem_message_with(problem, &|idx| format!("`{}`", format_type(idx)))
}

/// Shared implementation for problem message generation.
///
/// The `format_type` closure controls how `Idx` values are rendered:
/// - Simple path: `|idx| idx.display_name().to_string()` (no backticks)
/// - Rich path: `|idx| format!("`{}`", full_format(idx))` (with backticks)
pub(in crate::type_error::check_error) fn problem_message_with(
    problem: &TypeProblem,
    format_type: &dyn Fn(Idx) -> String,
) -> Option<String> {
    match problem {
        TypeProblem::NotCallable { actual_type } => Some(format!(
            "expected a function, found {}",
            format_type(*actual_type)
        )),
        TypeProblem::WrongArity { expected, found } => {
            let s = if *expected == 1 { "" } else { "s" };
            Some(format!("expected {expected} argument{s}, found {found}"))
        }
        TypeProblem::IntFloat { expected, found }
        | TypeProblem::NumericTypeMismatch { expected, found } => Some(format!(
            "expected `{expected}`, found `{found}`; use `{expected}(x)` to convert"
        )),
        TypeProblem::NumberToString => {
            Some("cannot use number as string; use `str(x)` to convert".to_string())
        }
        TypeProblem::StringToNumber => {
            Some("cannot use string as number; use `int(x)` or `float(x)` to convert".to_string())
        }
        TypeProblem::ExpectedList { .. } => {
            Some("expected a list; wrap the value in a list: `[x]`".to_string())
        }
        TypeProblem::ExpectedOption => Some("expected an Option type".to_string()),
        TypeProblem::NeedsUnwrap { inner_type } => Some(format!(
            "value needs to be unwrapped; inner type is {}",
            format_type(*inner_type)
        )),
        TypeProblem::ReturnMismatch { expected, found } => Some(format!(
            "return type mismatch: expected {}, found {}",
            format_type(*expected),
            format_type(*found)
        )),
        TypeProblem::ArgumentMismatch {
            arg_index,
            expected,
            found,
        } => Some(format!(
            "argument {} has type {}, expected {}",
            arg_index + 1,
            format_type(*found),
            format_type(*expected)
        )),
        TypeProblem::BadOperandType {
            op,
            op_category,
            found_type,
            required_type,
        } => {
            if *op_category == "unary" {
                Some(format!("cannot apply `{op}` to `{found_type}`"))
            } else {
                Some(format!(
                    "left operand of {op_category} operator must be `{required_type}`"
                ))
            }
        }
        TypeProblem::ClosureSelfCapture => Some("closure cannot capture itself".to_string()),
        _ => None,
    }
}
