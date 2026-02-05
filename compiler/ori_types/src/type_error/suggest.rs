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
//! # Example
//!
//! ```text
//! TypeProblem::IntFloat generates:
//!   - "Use `to_float()` to convert int to float" (priority 1)
//!   - "Use `to_int()` to convert float to int (truncates)" (priority 2)
//! ```

use ori_ir::Span;

use super::TypeProblem;

/// A suggestion for fixing a type error.
///
/// # Salsa Compatibility
/// Derives `Eq, PartialEq, Hash` for use in Salsa query results.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct Suggestion {
    /// Human-readable message describing the suggestion.
    pub message: String,
    /// Optional code replacement.
    pub replacement: Option<Replacement>,
    /// Priority (lower = more likely to be relevant).
    /// 0 = most likely, 1 = likely, 2 = possible, 3 = unlikely
    pub priority: u8,
}

impl Suggestion {
    /// Create a new suggestion without a code replacement.
    pub fn new(message: impl Into<String>, priority: u8) -> Self {
        Self {
            message: message.into(),
            replacement: None,
            priority,
        }
    }

    /// Create a new suggestion with a code replacement.
    pub fn with_replacement(
        message: impl Into<String>,
        priority: u8,
        span: Span,
        new_text: impl Into<String>,
    ) -> Self {
        Self {
            message: message.into(),
            replacement: Some(Replacement {
                span,
                new_text: new_text.into(),
            }),
            priority,
        }
    }

    /// Create a "did you mean" suggestion (priority 0).
    pub fn did_you_mean(suggestion: impl Into<String>) -> Self {
        Self::new(format!("did you mean `{}`?", suggestion.into()), 0)
    }

    /// Create a suggestion to use a specific function/method (priority 1).
    pub fn use_function(func_name: &str, description: &str) -> Self {
        Self::new(format!("use `{func_name}` {description}"), 1)
    }

    /// Create a suggestion to wrap in something (priority 1).
    pub fn wrap_in(wrapper: &str, example: &str) -> Self {
        Self::new(format!("wrap the value in `{wrapper}`: `{example}`"), 1)
    }
}

/// A code replacement suggestion.
///
/// # Salsa Compatibility
/// Derives `Eq, PartialEq, Hash` for use in Salsa query results.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct Replacement {
    /// Span of code to replace.
    pub span: Span,
    /// New code to insert.
    pub new_text: String,
}

impl Replacement {
    /// Create a new replacement.
    pub fn new(span: Span, new_text: impl Into<String>) -> Self {
        Self {
            span,
            new_text: new_text.into(),
        }
    }
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
                Suggestion::new(
                    "use `int.parse(str)` to parse string as int",
                    1,
                ),
                Suggestion::new(
                    "use `float.parse(str)` to parse string as float",
                    2,
                ),
            ],

            Self::NumericTypeMismatch { expected, found } => vec![Suggestion::new(
                format!("convert from `{found}` to `{expected}` explicitly"),
                1,
            )],

            // === Collection Problems ===
            Self::ExpectedList { found } => vec![
                Suggestion::wrap_in("list", "[value]"),
                Suggestion::new(
                    format!("`{found}` is not iterable; consider using a different approach"),
                    2,
                ),
            ],

            Self::ListElementMismatch { index } => vec![Suggestion::new(
                format!(
                    "check the type of element at index {index} - all list elements must have the same type"
                ),
                1,
            )],

            Self::ExpectedOption => {
                vec![Suggestion::wrap_in("Some", "Some(value)")]
            }

            Self::NeedsUnwrap { .. } => vec![
                Suggestion::new(
                    "use `?` to propagate `none` to the caller",
                    1,
                ),
                Suggestion::new(
                    "use `match` to handle both `Some` and `None` cases",
                    2,
                ),
                Suggestion::new(
                    "use `.unwrap()` if you're certain the value exists (will panic on None)",
                    3,
                ),
            ],

            Self::WrongCollectionType { expected, found } => vec![
                Suggestion::new(
                    format!("convert from `{found}` to `{expected}`"),
                    1,
                ),
                Suggestion::new(
                    format!("use `.to_{expected}()` if available"),
                    2,
                ),
            ],

            Self::MapKeyMismatch | Self::MapValueMismatch => vec![Suggestion::new(
                "check map key/value types match the expected types",
                1,
            )],

            // === Function Problems ===
            Self::WrongArity { expected, found } => {
                if *found > *expected {
                    let diff = found - expected;
                    let s = if diff == 1 { "" } else { "s" };
                    vec![Suggestion::new(
                        format!("remove {diff} extra argument{s}"),
                        0,
                    )]
                } else {
                    let diff = expected - found;
                    let s = if diff == 1 { "" } else { "s" };
                    vec![Suggestion::new(
                        format!("add {diff} missing argument{s}"),
                        0,
                    )]
                }
            }

            Self::ArgumentMismatch {
                arg_index,
                expected: _,
                found: _,
            } => vec![Suggestion::new(
                format!(
                    "check argument {} - it has the wrong type",
                    arg_index + 1
                ),
                1,
            )],

            Self::ReturnMismatch { .. } => vec![Suggestion::new(
                "the function's return value doesn't match the declared return type",
                1,
            )],

            Self::NotCallable { .. } => vec![
                Suggestion::new(
                    "only functions can be called with `()`",
                    1,
                ),
                Suggestion::new(
                    "if this is a struct, use `StructName { ... }` syntax instead",
                    2,
                ),
            ],

            Self::MissingArguments { missing } => {
                let names: Vec<_> = missing.iter().map(|n| format!("`{n:?}`")).collect();
                vec![Suggestion::new(
                    format!("add missing arguments: {}", names.join(", ")),
                    0,
                )]
            }

            Self::ExtraArguments { count } => {
                let s = if *count == 1 { "" } else { "s" };
                vec![Suggestion::new(
                    format!("remove {count} extra argument{s}"),
                    0,
                )]
            }

            // === Record/Struct Problems ===
            Self::MissingField {
                field_name,
                available,
            } => {
                let mut suggestions = vec![Suggestion::new(
                    format!("add the missing field `{field_name:?}`"),
                    1,
                )];

                if !available.is_empty() {
                    let names: Vec<_> = available.iter().take(5).map(|n| format!("`{n:?}`")).collect();
                    suggestions.push(Suggestion::new(
                        format!("available fields: {}", names.join(", ")),
                        2,
                    ));
                }

                suggestions
            }

            Self::ExtraField { field_name } => vec![Suggestion::new(
                format!("remove the extra field `{field_name:?}`"),
                0,
            )],

            Self::FieldTypeMismatch { field_name, .. } => vec![Suggestion::new(
                format!("check the type of field `{field_name:?}`"),
                1,
            )],

            Self::FieldTypo {
                attempted: _,
                suggestion,
                distance: _,
            } => vec![Suggestion::did_you_mean(format!("{suggestion:?}"))],

            Self::WrongRecordType { expected, found } => vec![Suggestion::new(
                format!("expected type `{expected:?}`, got `{found:?}`"),
                1,
            )],

            // === Type Variable Problems ===
            Self::RigidMismatch { rigid_name, .. } => vec![
                Suggestion::new(
                    format!(
                        "type parameter `{rigid_name:?}` cannot be unified with a concrete type"
                    ),
                    1,
                ),
                Suggestion::new(
                    "consider using a more specific type annotation",
                    2,
                ),
            ],

            Self::InfiniteType { .. } => vec![
                Suggestion::new(
                    "this creates a self-referential type that has no finite representation",
                    1,
                ),
                Suggestion::new(
                    "use a newtype wrapper to break the cycle: `type Wrapper = { inner: T }`",
                    2,
                ),
            ],

            Self::EscapingVariable { var_name } => {
                let name_info = match var_name {
                    Some(name) => format!(" (`{name:?}`)"),
                    None => String::new(),
                };
                vec![Suggestion::new(
                    format!(
                        "type variable{name_info} escapes its scope; add a type annotation to fix it"
                    ),
                    1,
                )]
            }

            // === Capability Problems ===
            Self::MissingCapability { required } => vec![Suggestion::new(
                format!("add `uses {required:?}` to the function signature"),
                0,
            )],

            Self::CapabilityConflict { provided, required } => vec![Suggestion::new(
                format!(
                    "capability `{provided:?}` doesn't satisfy requirement `{required:?}`"
                ),
                1,
            )],

            // === Pattern Problems ===
            Self::PatternMismatch { .. } => vec![Suggestion::new(
                "the pattern doesn't match the type being matched against",
                1,
            )],

            Self::NonExhaustiveMatch { missing_patterns } => {
                let patterns: String = missing_patterns.iter().take(5).cloned().collect::<Vec<_>>().join(", ");
                vec![
                    Suggestion::new(
                        format!("add patterns for: {patterns}"),
                        0,
                    ),
                    Suggestion::new(
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
                    vec![Suggestion::new(
                        format!("operator `{op}` can only be applied to `{required_type}`"),
                        0,
                    )]
                } else {
                    vec![Suggestion::new(
                        format!("{op_category} operators require `{required_type}` operands"),
                        0,
                    )]
                }
            }

            Self::ClosureSelfCapture => vec![Suggestion::new(
                "use recursion through named functions instead",
                0,
            )],

            // === Generic Fallback ===
            Self::TypeMismatch {
                expected_category,
                found_category,
            } => vec![Suggestion::new(
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
mod tests {
    use super::*;

    #[test]
    fn int_float_suggestions() {
        let problem = TypeProblem::IntFloat;
        let suggestions = problem.suggestions();
        assert_eq!(suggestions.len(), 2);
        assert!(suggestions[0].message.contains("to_float"));
    }

    #[test]
    fn needs_unwrap_suggestions() {
        let problem = TypeProblem::NeedsUnwrap {
            inner_type: crate::Idx::INT,
        };
        let suggestions = problem.suggestions();
        assert!(!suggestions.is_empty());
        assert!(suggestions[0].message.contains('?'));
    }

    #[test]
    fn wrong_arity_suggestions() {
        let problem = TypeProblem::WrongArity {
            expected: 2,
            found: 4,
        };
        let suggestions = problem.suggestions();
        assert_eq!(suggestions.len(), 1);
        assert!(suggestions[0].message.contains("remove"));
        assert!(suggestions[0].message.contains('2'));
    }

    #[test]
    fn suggestion_priority_sorting() {
        let problem = TypeProblem::NeedsUnwrap {
            inner_type: crate::Idx::INT,
        };
        let suggestions = problem.suggestions();

        // Check that suggestions are sorted by priority
        for i in 1..suggestions.len() {
            assert!(suggestions[i - 1].priority <= suggestions[i].priority);
        }
    }

    #[test]
    fn top_suggestion() {
        let problem = TypeProblem::IntFloat;
        let top = problem.top_suggestion();
        assert!(top.is_some_and(|s| s.contains("to_float")));
    }
}
