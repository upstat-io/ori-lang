//! Suggestion generation for type errors.
//!
//! This module generates actionable suggestions for fixing type problems.
//! Each `TypeProblem` knows what suggestions to generate based on the
//! specific error type.
//!
//! # Design
//!
//! Based on Elm's approach:
//! - Suggestions have priority (lower = more likely to be relevant)
//! - Some suggestions include code replacements
//! - Problems generate multiple suggestions when appropriate
//!
//! Suggestions use `ori_diagnostic::Suggestion` directly (unified type).
//!
//! # Example
//!
//! ```text
//! TypeProblem::IntFloat generates:
//!   - "Use `to_float()` to convert int to float" (priority 1)
//!   - "Use `to_int()` to convert float to int (truncates)" (priority 2)
//! ```

use ori_diagnostic::Suggestion;

use super::TypeProblem;

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
        match self {
            // === Numeric Problems ===
            Self::IntFloat => vec![
                Suggestion::use_function("to_float()", "to convert int to float"),
                Suggestion::use_function("to_int()", "to convert float to int (truncates)"),
            ],

            Self::NumberToString => vec![Suggestion::use_function(
                "to_str()",
                "to convert a number to string",
            )],

            Self::StringToNumber => vec![
                Suggestion::text(
                    "use `int.parse(str)` to parse string as int",
                    1,
                ),
                Suggestion::text(
                    "use `float.parse(str)` to parse string as float",
                    2,
                ),
            ],

            Self::NumericTypeMismatch { expected, found } => vec![Suggestion::text(
                format!("convert from `{found}` to `{expected}` explicitly"),
                1,
            )],

            // === Collection Problems ===
            Self::ExpectedList { found } => vec![
                Suggestion::wrap_in("list", "[value]"),
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

            Self::ExpectedOption => {
                vec![Suggestion::wrap_in("Some", "Some(value)")]
            }

            Self::NeedsUnwrap { .. } => vec![
                Suggestion::text(
                    "use `?` to propagate `none` to the caller",
                    1,
                ),
                Suggestion::text(
                    "use `match` to handle both `Some` and `None` cases",
                    2,
                ),
                Suggestion::text(
                    "use `.unwrap()` if you're certain the value exists (will panic on None)",
                    3,
                ),
            ],

            Self::WrongCollectionType { expected, found } => vec![
                Suggestion::text(
                    format!("convert from `{found}` to `{expected}`"),
                    1,
                ),
                Suggestion::text(
                    format!("use `.to_{expected}()` if available"),
                    2,
                ),
            ],

            Self::MapKeyMismatch | Self::MapValueMismatch => vec![Suggestion::text(
                "check map key/value types match the expected types",
                1,
            )],

            // === Function Problems ===
            Self::WrongArity { expected, found } => {
                if *found > *expected {
                    let diff = found - expected;
                    let s = if diff == 1 { "" } else { "s" };
                    vec![Suggestion::text(
                        format!("remove {diff} extra argument{s}"),
                        0,
                    )]
                } else {
                    let diff = expected - found;
                    let s = if diff == 1 { "" } else { "s" };
                    vec![Suggestion::text(
                        format!("add {diff} missing argument{s}"),
                        0,
                    )]
                }
            }

            Self::ArgumentMismatch {
                arg_index,
                expected: _,
                found: _,
            } => vec![Suggestion::text(
                format!(
                    "check argument {} - it has the wrong type",
                    arg_index + 1
                ),
                1,
            )],

            Self::ReturnMismatch { .. } => vec![Suggestion::text(
                "the function's return value doesn't match the declared return type",
                1,
            )],

            Self::NotCallable { .. } => vec![
                Suggestion::text(
                    "only functions can be called with `()`",
                    1,
                ),
                Suggestion::text(
                    "if this is a struct, use `StructName { ... }` syntax instead",
                    2,
                ),
            ],

            Self::MissingArguments { missing } => {
                let names: Vec<_> = missing.iter().map(|n| format!("`{n:?}`")).collect();
                vec![Suggestion::text(
                    format!("add missing arguments: {}", names.join(", ")),
                    0,
                )]
            }

            Self::ExtraArguments { count } => {
                let s = if *count == 1 { "" } else { "s" };
                vec![Suggestion::text(
                    format!("remove {count} extra argument{s}"),
                    0,
                )]
            }

            // === Record/Struct Problems ===
            Self::MissingField {
                field_name,
                available,
            } => {
                let mut suggestions = vec![Suggestion::text(
                    format!("add the missing field `{field_name:?}`"),
                    1,
                )];

                if !available.is_empty() {
                    let names: Vec<_> = available.iter().take(5).map(|n| format!("`{n:?}`")).collect();
                    suggestions.push(Suggestion::text(
                        format!("available fields: {}", names.join(", ")),
                        2,
                    ));
                }

                suggestions
            }

            Self::ExtraField { field_name } => vec![Suggestion::text(
                format!("remove the extra field `{field_name:?}`"),
                0,
            )],

            Self::FieldTypeMismatch { field_name, .. } => vec![Suggestion::text(
                format!("check the type of field `{field_name:?}`"),
                1,
            )],

            Self::FieldTypo {
                attempted: _,
                suggestion,
                distance: _,
            } => vec![Suggestion::did_you_mean(format!("{suggestion:?}"))],

            Self::WrongRecordType { expected, found } => vec![Suggestion::text(
                format!("expected type `{expected:?}`, got `{found:?}`"),
                1,
            )],

            // === Type Variable Problems ===
            Self::RigidMismatch { rigid_name, .. } => vec![
                Suggestion::text(
                    format!(
                        "type parameter `{rigid_name:?}` cannot be unified with a concrete type"
                    ),
                    1,
                ),
                Suggestion::text(
                    "consider using a more specific type annotation",
                    2,
                ),
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
                let name_info = match var_name {
                    Some(name) => format!(" (`{name:?}`)"),
                    None => String::new(),
                };
                vec![Suggestion::text(
                    format!(
                        "type variable{name_info} escapes its scope; add a type annotation to fix it"
                    ),
                    1,
                )]
            }

            // === Capability Problems ===
            Self::MissingCapability { required } => vec![Suggestion::text(
                format!("add `uses {required:?}` to the function signature"),
                0,
            )],

            Self::CapabilityConflict { provided, required } => vec![Suggestion::text(
                format!(
                    "capability `{provided:?}` doesn't satisfy requirement `{required:?}`"
                ),
                1,
            )],

            // === Pattern Problems ===
            Self::PatternMismatch { .. } => vec![Suggestion::text(
                "the pattern doesn't match the type being matched against",
                1,
            )],

            Self::NonExhaustiveMatch { missing_patterns } => {
                let patterns: String = missing_patterns.iter().take(5).cloned().collect::<Vec<_>>().join(", ");
                vec![
                    Suggestion::text(
                        format!("add patterns for: {patterns}"),
                        0,
                    ),
                    Suggestion::text(
                        "or add a catch-all pattern: `_ =>`",
                        1,
                    ),
                ]
            }

            // === Operator Problems ===
            Self::BadOperandType {
                op,
                op_category,
                found_type: _,
                required_type,
            } => {
                if *op_category == "unary" {
                    vec![Suggestion::text(
                        format!("operator `{op}` can only be applied to `{required_type}`"),
                        0,
                    )]
                } else {
                    vec![Suggestion::text(
                        format!("{op_category} operators require `{required_type}` operands"),
                        0,
                    )]
                }
            }

            Self::ClosureSelfCapture => vec![Suggestion::text(
                "use recursion through named functions instead",
                0,
            )],

            // === Generic Fallback ===
            Self::TypeMismatch {
                expected_category,
                found_category,
            } => vec![Suggestion::text(
                format!("expected `{expected_category}`, found `{found_category}`"),
                1,
            )],
        }
    }

    /// Get the highest-priority suggestion message, if any.
    pub fn top_suggestion(&self) -> Option<String> {
        self.suggestions().first().map(|s| s.message.clone())
    }
}

#[cfg(test)]
mod tests;
