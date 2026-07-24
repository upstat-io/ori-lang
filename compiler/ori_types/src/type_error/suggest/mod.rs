//! Suggestion generation for type errors.
//!
//! This module generates actionable suggestions for fixing type problems.
//! Each `TypeProblem` knows what suggestions to generate based on the
//! specific error type.
//!
//! # Design
//!
//! Historical influence: the Elm suggestion SHAPE:
//! - Suggestions have priority (lower = more likely to be relevant)
//! - Some suggestions include code replacements
//! - Problems generate multiple suggestions when appropriate
//!
//! Suggestions use `ori_diagnostic::Suggestion` directly (unified type).
//!
//! # Example
//!
//! ```text
//! TypeProblem::IntFloat { expected: "float", .. } generates:
//!   - "use `float(x)` to convert" (priority 1)
//! ```

use ori_diagnostic::Suggestion;
use ori_ir::Name;

use super::TypeProblem;

/// Comma-joined list of `n` backtick-wrapped positional placeholders
/// (`` `{0}`, `{1}`, … ``) for a `text_with_names` message carrying `n` names.
fn numbered_placeholders(n: usize) -> String {
    (0..n)
        .map(|i| format!("`{{{i}}}`"))
        .collect::<Vec<_>>()
        .join(", ")
}

impl TypeProblem {
    /// Generate suggestions for fixing this problem.
    ///
    /// Returns a list of suggestions sorted by priority (most likely first).
    pub fn suggestions(&self) -> Vec<Suggestion> {
        let mut suggestions = self.generate_suggestions();
        suggestions.sort_by_key(|s| s.priority);
        suggestions
    }

    fn generate_suggestions(&self) -> Vec<Suggestion> {
        if let Some(suggestions) = self.numeric_suggestions() {
            return suggestions;
        }
        if let Some(suggestions) = self.collection_suggestions() {
            return suggestions;
        }
        if let Some(suggestions) = self.function_suggestions() {
            return suggestions;
        }
        if let Some(suggestions) = self.record_suggestions() {
            return suggestions;
        }
        if let Some(suggestions) = self.inference_suggestions() {
            return suggestions;
        }
        if let Some(suggestions) = self.language_suggestions() {
            return suggestions;
        }
        match self {
            Self::IntFloat { .. }
            | Self::NumericTypeMismatch { .. }
            | Self::NumberToString
            | Self::StringToNumber
            | Self::ExpectedList { .. }
            | Self::ListElementMismatch { .. }
            | Self::ExpectedOption
            | Self::NeedsUnwrap { .. }
            | Self::WrongCollectionType { .. }
            | Self::MapKeyMismatch
            | Self::MapValueMismatch
            | Self::WrongArity { .. }
            | Self::ArgumentMismatch { .. }
            | Self::ReturnMismatch { .. }
            | Self::NotCallable { .. }
            | Self::MissingArguments { .. }
            | Self::ExtraArguments { .. }
            | Self::MissingField { .. }
            | Self::ExtraField { .. }
            | Self::FieldTypeMismatch { .. }
            | Self::FieldTypo { .. }
            | Self::WrongRecordType { .. }
            | Self::RigidMismatch { .. }
            | Self::InfiniteType { .. }
            | Self::EscapingVariable { .. }
            | Self::MissingCapability { .. }
            | Self::CapabilityConflict { .. }
            | Self::PatternMismatch { .. }
            | Self::NonExhaustiveMatch { .. }
            | Self::BadOperandType { .. }
            | Self::ClosureSelfCapture
            | Self::TypeMismatch { .. } => unreachable!("handled by category helper"),
        }
    }

    fn inference_suggestions(&self) -> Option<Vec<Suggestion>> {
        let suggestions = match self {
            Self::RigidMismatch { rigid_name, .. } => vec![
                Suggestion::text_with_names(
                    "type parameter `{0}` cannot be unified with a concrete type",
                    vec![*rigid_name],
                    1,
                ),
                Suggestion::text("consider using a more specific type annotation", 2),
            ],
            Self::InfiniteType { .. } => vec![
                Suggestion::text(
                    "this creates a self-referential type that has no finite representation",
                    1,
                ),
                Suggestion::text(
                    "use a newtype wrapper to break the cycle: `type Wrapper = { inner: T }`",
                    2,
                ),
            ],
            Self::EscapingVariable { var_name } => {
                let (message, names): (&str, Vec<Name>) = match var_name {
                    Some(name) => (
                        "type variable (`{0}`) escapes its scope; add a type annotation to fix it",
                        vec![*name],
                    ),
                    None => (
                        "type variable escapes its scope; add a type annotation to fix it",
                        Vec::new(),
                    ),
                };
                vec![Suggestion::text_with_names(message, names, 1)]
            }
            _ => return None,
        };
        Some(suggestions)
    }

    fn language_suggestions(&self) -> Option<Vec<Suggestion>> {
        let suggestions = match self {
            Self::MissingCapability { required } => vec![Suggestion::text_with_names(
                "add `uses {0}` to the function signature",
                vec![*required],
                0,
            )],
            Self::CapabilityConflict { provided, required } => vec![Suggestion::text_with_names(
                "capability `{0}` doesn't satisfy requirement `{1}`",
                vec![*provided, *required],
                1,
            )],
            Self::PatternMismatch { .. } => vec![Suggestion::text(
                "the pattern doesn't match the type being matched against",
                1,
            )],
            Self::NonExhaustiveMatch { missing_patterns } => {
                let patterns = missing_patterns
                    .iter()
                    .take(5)
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(", ");
                vec![
                    Suggestion::text(format!("add patterns for: {patterns}"), 0),
                    Suggestion::text("or add a catch-all pattern: `_ =>`", 1),
                ]
            }
            Self::BadOperandType {
                op,
                op_category,
                required_type,
                ..
            } => {
                let message = if *op_category == "unary" {
                    format!("operator `{op}` can only be applied to `{required_type}`")
                } else {
                    format!("{op_category} operators require `{required_type}` operands")
                };
                vec![Suggestion::text(message, 0)]
            }
            Self::ClosureSelfCapture => vec![Suggestion::text(
                "use recursion through named functions instead",
                0,
            )],
            Self::TypeMismatch {
                expected_category,
                found_category,
            } => vec![Suggestion::text(
                format!("expected `{expected_category}`, found `{found_category}`"),
                1,
            )],
            _ => return None,
        };
        Some(suggestions)
    }

    fn numeric_suggestions(&self) -> Option<Vec<Suggestion>> {
        let suggestions = match self {
            Self::IntFloat { expected, .. } | Self::NumericTypeMismatch { expected, .. } => {
                vec![Suggestion::text(
                    format!("use `{expected}(x)` to convert"),
                    1,
                )]
            }
            Self::NumberToString => vec![Suggestion::text(
                "use `str(x)` to convert the value to a string",
                1,
            )],
            Self::StringToNumber => vec![
                Suggestion::text("use `int(x)` to convert string to int", 1),
                Suggestion::text("use `float(x)` to convert string to float", 2),
            ],
            _ => return None,
        };
        Some(suggestions)
    }

    fn collection_suggestions(&self) -> Option<Vec<Suggestion>> {
        let suggestions = match self {
            Self::ExpectedList { found } => vec![
                Suggestion::text("wrap the value in a list: `[x]`", 1),
                Suggestion::text(
                    format!("`{found}` is not iterable; consider using a different approach"),
                    2,
                ),
            ],
            Self::ListElementMismatch { index } => vec![Suggestion::text(
                format!(
                    "check the type of element at index {index} - all list elements must have the same type"
                ),
                1,
            )],
            Self::ExpectedOption => vec![Suggestion::wrap_in("Some", "Some(value)")],
            Self::NeedsUnwrap { .. } => vec![
                Suggestion::text("use `?` to propagate `none` to the caller", 1),
                Suggestion::text("use `match` to handle both `Some` and `None` cases", 2),
                Suggestion::text(
                    "use `.unwrap()` if you're certain the value exists (will panic on None)",
                    3,
                ),
            ],
            Self::WrongCollectionType { expected, found } => vec![
                Suggestion::text(format!("convert from `{found}` to `{expected}`"), 1),
                Suggestion::text(format!("use `.to_{expected}()` if available"), 2),
            ],
            Self::MapKeyMismatch | Self::MapValueMismatch => vec![Suggestion::text(
                "check map key/value types match the expected types",
                1,
            )],
            _ => return None,
        };
        Some(suggestions)
    }

    fn function_suggestions(&self) -> Option<Vec<Suggestion>> {
        let suggestions = match self {
            Self::WrongArity { expected, found } => {
                let (action, qualifier, difference) = if found > expected {
                    ("remove", "extra ", found - expected)
                } else {
                    ("add", "missing ", expected - found)
                };
                let plural = if difference == 1 { "" } else { "s" };
                vec![Suggestion::text(
                    format!("{action} {difference} {qualifier}argument{plural}"),
                    0,
                )]
            }
            Self::ArgumentMismatch { arg_index, .. } => vec![Suggestion::text(
                format!("check argument {} - it has the wrong type", arg_index + 1),
                1,
            )],
            Self::ReturnMismatch { .. } => vec![Suggestion::text(
                "the function's return value doesn't match the declared return type",
                1,
            )],
            Self::NotCallable { .. } => vec![
                Suggestion::text("only functions can be called with `()`", 1),
                Suggestion::text(
                    "if this is a struct, use `StructName { ... }` syntax instead",
                    2,
                ),
            ],
            Self::MissingArguments { missing } => vec![Suggestion::text_with_names(
                format!(
                    "add missing arguments: {}",
                    numbered_placeholders(missing.len())
                ),
                missing.clone(),
                0,
            )],
            Self::ExtraArguments { count } => {
                let plural = if *count == 1 { "" } else { "s" };
                vec![Suggestion::text(
                    format!("remove {count} extra argument{plural}"),
                    0,
                )]
            }
            _ => return None,
        };
        Some(suggestions)
    }

    fn record_suggestions(&self) -> Option<Vec<Suggestion>> {
        let suggestions = match self {
            Self::MissingField {
                field_name,
                available,
            } => {
                let mut suggestions = vec![Suggestion::text_with_names(
                    "add the missing field `{0}`",
                    vec![*field_name],
                    1,
                )];
                if !available.is_empty() {
                    let shown: Vec<Name> = available.iter().take(5).copied().collect();
                    suggestions.push(Suggestion::text_with_names(
                        format!("available fields: {}", numbered_placeholders(shown.len())),
                        shown,
                        2,
                    ));
                }
                suggestions
            }
            Self::ExtraField { field_name } => vec![Suggestion::text_with_names(
                "remove the extra field `{0}`",
                vec![*field_name],
                0,
            )],
            Self::FieldTypeMismatch { field_name, .. } => vec![Suggestion::text_with_names(
                "check the type of field `{0}`",
                vec![*field_name],
                1,
            )],
            Self::FieldTypo { suggestion, .. } => vec![Suggestion::text_with_names(
                "did you mean `{0}`?",
                vec![*suggestion],
                0,
            )],
            Self::WrongRecordType { expected, found } => vec![Suggestion::text_with_names(
                "expected type `{0}`, got `{1}`",
                vec![*expected, *found],
                1,
            )],
            _ => return None,
        };
        Some(suggestions)
    }
}

#[cfg(test)]
mod tests;
