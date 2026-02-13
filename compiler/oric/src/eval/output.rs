//! Salsa-compatible evaluation output types.
//!
//! These types are designed for use in Salsa queries, requiring
//! Clone + Eq + `PartialEq` + Hash + Debug traits.

use super::value::Value;
use crate::ir::{Name, StringInterner};
use ori_diagnostic::ErrorCode;
use ori_ir::Span;
use std::hash::{Hash, Hasher};

/// Salsa-compatible representation of an evaluated value.
///
/// Unlike `Value`, this type has Clone + Eq + Hash for use in Salsa queries.
/// Complex values (functions, structs) are represented as strings.
///
/// # Salsa Compatibility
/// Has all required traits: Clone, Eq, `PartialEq`, Hash, Debug
#[derive(Clone, Debug)]
pub enum EvalOutput {
    /// Integer value.
    Int(i64),
    /// Floating-point value (stored as bits for Hash).
    Float(u64),
    /// Boolean value.
    Bool(bool),
    /// String value.
    Str(String),
    /// Character value.
    Char(char),
    /// Byte value.
    Byte(u8),
    /// Void (unit) value.
    Void,
    /// List of values.
    List(Vec<EvalOutput>),
    /// Tuple of values.
    Tuple(Vec<EvalOutput>),
    /// Option: Some(value).
    Some(Box<EvalOutput>),
    /// Option: None.
    None,
    /// Result: Ok(value).
    Ok(Box<EvalOutput>),
    /// Result: Err(error).
    Err(Box<EvalOutput>),
    /// Duration in nanoseconds.
    Duration(i64),
    /// Size in bytes.
    Size(u64),
    /// Ordering value (Less=0, Equal=1, Greater=2).
    Ordering(i8),
    /// Range value.
    Range {
        start: i64,
        end: i64,
        inclusive: bool,
    },
    /// Function (not directly representable in Salsa; carries structured metadata).
    Function {
        description: String,
        /// Number of parameters, when known.
        arity: Option<usize>,
    },
    /// Struct (not directly representable in Salsa; carries structured metadata).
    Struct {
        description: String,
        /// Number of fields, when known.
        field_count: Option<usize>,
    },
    /// User-defined variant.
    Variant {
        type_name: Name,
        variant_name: Name,
        fields: Vec<EvalOutput>,
    },
    /// Map (stored as key-value pairs).
    Map(Vec<(String, EvalOutput)>),
    /// Error during evaluation.
    Error(String),
}

impl EvalOutput {
    /// Convert a runtime Value to a Salsa-compatible `EvalOutput`.
    pub fn from_value(value: &Value, interner: &StringInterner) -> Self {
        match value {
            Value::Int(n) => EvalOutput::Int(n.raw()),
            Value::Float(f) => EvalOutput::Float(f.to_bits()),
            Value::Bool(b) => EvalOutput::Bool(*b),
            Value::Str(s) => EvalOutput::Str(s.to_string()),
            Value::Char(c) => EvalOutput::Char(*c),
            Value::Byte(b) => EvalOutput::Byte(*b),
            Value::Void => EvalOutput::Void,
            Value::List(items) => EvalOutput::List(
                items
                    .iter()
                    .map(|v| Self::from_value(v, interner))
                    .collect(),
            ),
            Value::Tuple(items) => EvalOutput::Tuple(
                items
                    .iter()
                    .map(|v| Self::from_value(v, interner))
                    .collect(),
            ),
            Value::Some(v) => EvalOutput::Some(Box::new(Self::from_value(v, interner))),
            Value::None => EvalOutput::None,
            Value::Ok(v) => EvalOutput::Ok(Box::new(Self::from_value(v, interner))),
            Value::Err(v) => EvalOutput::Err(Box::new(Self::from_value(v, interner))),
            Value::Duration(ms) => EvalOutput::Duration(*ms),
            Value::Size(bytes) => EvalOutput::Size(*bytes),
            Value::Ordering(ord) => EvalOutput::Ordering(ord.to_tag()),
            Value::Range(r) => EvalOutput::Range {
                start: r.start,
                end: r.end,
                inclusive: r.inclusive,
            },
            Value::Function(f) => {
                let arity = f.params.len();
                EvalOutput::Function {
                    description: format!("<function with {arity} params>"),
                    arity: Some(arity),
                }
            }
            Value::MemoizedFunction(mf) => {
                let arity = mf.func.params.len();
                EvalOutput::Function {
                    description: format!("<memoized function with {arity} params>"),
                    arity: Some(arity),
                }
            }
            Value::FunctionVal(_, name) => EvalOutput::Function {
                description: format!("<{name}>"),
                arity: None,
            },
            Value::Struct(s) => EvalOutput::Struct {
                description: format!("<struct {}>", interner.lookup(s.name())),
                field_count: Some(s.fields.len()),
            },
            Value::Variant {
                type_name,
                variant_name,
                fields,
            } => EvalOutput::Variant {
                type_name: *type_name,
                variant_name: *variant_name,
                fields: fields
                    .iter()
                    .map(|v| Self::from_value(v, interner))
                    .collect(),
            },
            Value::VariantConstructor {
                type_name,
                variant_name,
                field_count,
            } => {
                let type_str = interner.lookup(*type_name);
                let variant_str = interner.lookup(*variant_name);
                EvalOutput::Function {
                    description: format!(
                        "<{type_str}::{variant_str} constructor ({field_count} fields)>"
                    ),
                    arity: Some(*field_count),
                }
            }
            Value::Newtype { type_name, inner } => {
                // Display newtype by showing the wrapped value
                let type_str = interner.lookup(*type_name);
                let inner_output = Self::from_value(inner, interner);
                EvalOutput::Struct {
                    description: format!("{type_str}({inner_output:?})"),
                    field_count: Some(1),
                }
            }
            Value::NewtypeConstructor { type_name } => {
                let type_str = interner.lookup(*type_name);
                EvalOutput::Function {
                    description: format!("<{type_str} constructor>"),
                    arity: Some(1),
                }
            }
            Value::Map(map) => {
                let entries: Vec<_> = map
                    .iter()
                    .map(|(k, v)| (k.clone(), Self::from_value(v, interner)))
                    .collect();
                EvalOutput::Map(entries)
            }
            Value::ModuleNamespace(ns) => EvalOutput::Function {
                description: format!("<module namespace with {} items>", ns.len()),
                arity: None,
            },
            Value::Error(msg) => EvalOutput::Error(msg.clone()),
            Value::TypeRef { type_name } => {
                let type_str = interner.lookup(*type_name);
                EvalOutput::Function {
                    description: format!("<type {type_str}>"),
                    arity: None,
                }
            }
        }
    }

    /// Get a display string for this output.
    ///
    /// Requires the interner to resolve interned names (for variants).
    pub fn display(&self, interner: &StringInterner) -> String {
        match self {
            EvalOutput::Int(n) => n.to_string(),
            EvalOutput::Float(bits) => f64::from_bits(*bits).to_string(),
            EvalOutput::Bool(b) => b.to_string(),
            EvalOutput::Str(s) => format!("\"{s}\""),
            EvalOutput::Char(c) => format!("'{c}'"),
            EvalOutput::Byte(b) => format!("0x{b:02x}"),
            EvalOutput::Void => "void".to_string(),
            EvalOutput::List(items) => {
                let inner: Vec<_> = items.iter().map(|i| i.display(interner)).collect();
                format!("[{}]", inner.join(", "))
            }
            EvalOutput::Tuple(items) => {
                let inner: Vec<_> = items.iter().map(|i| i.display(interner)).collect();
                format!("({})", inner.join(", "))
            }
            EvalOutput::Some(v) => format!("Some({})", v.display(interner)),
            EvalOutput::None => "None".to_string(),
            EvalOutput::Ok(v) => format!("Ok({})", v.display(interner)),
            EvalOutput::Err(v) => format!("Err({})", v.display(interner)),
            EvalOutput::Duration(ms) => format!("{ms}ms"),
            EvalOutput::Size(bytes) => format!("{bytes}b"),
            EvalOutput::Ordering(tag) => match tag {
                0 => "Less".to_string(),
                1 => "Equal".to_string(),
                2 => "Greater".to_string(),
                _ => format!("Ordering({tag})"),
            },
            EvalOutput::Range {
                start,
                end,
                inclusive,
            } => {
                if *inclusive {
                    format!("{start}..={end}")
                } else {
                    format!("{start}..{end}")
                }
            }
            EvalOutput::Function { description, .. } | EvalOutput::Struct { description, .. } => {
                description.clone()
            }
            EvalOutput::Variant {
                type_name,
                variant_name,
                fields,
            } => {
                let type_str = interner.lookup(*type_name);
                let variant_str = interner.lookup(*variant_name);
                if fields.is_empty() {
                    format!("{type_str}::{variant_str}")
                } else {
                    let inner: Vec<_> = fields.iter().map(|f| f.display(interner)).collect();
                    format!("{type_str}::{variant_str}({})", inner.join(", "))
                }
            }
            EvalOutput::Map(entries) => {
                let inner: Vec<_> = entries
                    .iter()
                    .map(|(k, v)| format!("\"{}\": {}", k, v.display(interner)))
                    .collect();
                format!("{{{}}}", inner.join(", "))
            }
            EvalOutput::Error(msg) => format!("<error: {msg}>"),
        }
    }
}

impl PartialEq for EvalOutput {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            // i64 types (Int, Duration in nanoseconds)
            (EvalOutput::Int(a), EvalOutput::Int(b))
            | (EvalOutput::Duration(a), EvalOutput::Duration(b)) => a == b,
            (EvalOutput::Bool(a), EvalOutput::Bool(b)) => a == b,
            (EvalOutput::Byte(a), EvalOutput::Byte(b)) => a == b,
            // i8 types (Ordering tag)
            (EvalOutput::Ordering(a), EvalOutput::Ordering(b)) => a == b,
            (EvalOutput::Char(a), EvalOutput::Char(b)) => a == b,
            // u64 types (Float stored as bits, Size in bytes)
            (EvalOutput::Float(a), EvalOutput::Float(b))
            | (EvalOutput::Size(a), EvalOutput::Size(b)) => a == b,
            // String types can be merged
            (EvalOutput::Str(a), EvalOutput::Str(b))
            | (EvalOutput::Error(a), EvalOutput::Error(b)) => a == b,
            (
                EvalOutput::Function {
                    description: a,
                    arity: ar1,
                },
                EvalOutput::Function {
                    description: b,
                    arity: ar2,
                },
            ) => a == b && ar1 == ar2,
            (
                EvalOutput::Struct {
                    description: a,
                    field_count: fc1,
                },
                EvalOutput::Struct {
                    description: b,
                    field_count: fc2,
                },
            ) => a == b && fc1 == fc2,
            // Vec<EvalOutput> types can be merged
            (EvalOutput::List(a), EvalOutput::List(b))
            | (EvalOutput::Tuple(a), EvalOutput::Tuple(b)) => a == b,
            // Box<EvalOutput> types can be merged
            (EvalOutput::Some(a), EvalOutput::Some(b))
            | (EvalOutput::Ok(a), EvalOutput::Ok(b))
            | (EvalOutput::Err(a), EvalOutput::Err(b)) => a == b,
            (EvalOutput::Map(a), EvalOutput::Map(b)) => a == b,
            // Unit types
            (EvalOutput::Void, EvalOutput::Void) | (EvalOutput::None, EvalOutput::None) => true,
            // Range with multiple fields
            (
                EvalOutput::Range {
                    start: s1,
                    end: e1,
                    inclusive: i1,
                },
                EvalOutput::Range {
                    start: s2,
                    end: e2,
                    inclusive: i2,
                },
            ) => s1 == s2 && e1 == e2 && i1 == i2,
            // Variant with type, variant name, and fields
            (
                EvalOutput::Variant {
                    type_name: t1,
                    variant_name: v1,
                    fields: f1,
                },
                EvalOutput::Variant {
                    type_name: t2,
                    variant_name: v2,
                    fields: f2,
                },
            ) => t1 == t2 && v1 == v2 && f1 == f2,
            _ => false,
        }
    }
}

impl Eq for EvalOutput {}

impl Hash for EvalOutput {
    fn hash<H: Hasher>(&self, state: &mut H) {
        std::mem::discriminant(self).hash(state);
        match self {
            EvalOutput::Int(n) => n.hash(state),
            EvalOutput::Bool(b) => b.hash(state),
            EvalOutput::Char(c) => c.hash(state),
            EvalOutput::Byte(b) => b.hash(state),
            EvalOutput::Ordering(tag) => tag.hash(state),
            // u64 types
            EvalOutput::Float(bits) | EvalOutput::Size(bits) => {
                bits.hash(state);
            }
            // i64 types
            EvalOutput::Duration(ns) => ns.hash(state),
            // String types
            EvalOutput::Str(s) | EvalOutput::Error(s) => s.hash(state),
            EvalOutput::Function { description, arity } => {
                description.hash(state);
                arity.hash(state);
            }
            EvalOutput::Struct {
                description,
                field_count,
            } => {
                description.hash(state);
                field_count.hash(state);
            }
            // Vec<EvalOutput> types
            EvalOutput::List(items) | EvalOutput::Tuple(items) => items.hash(state),
            // Box<EvalOutput> types
            EvalOutput::Some(v) | EvalOutput::Ok(v) | EvalOutput::Err(v) => v.hash(state),
            EvalOutput::Map(entries) => entries.hash(state),
            // Unit types
            EvalOutput::Void | EvalOutput::None => {}
            EvalOutput::Range {
                start,
                end,
                inclusive,
            } => {
                start.hash(state);
                end.hash(state);
                inclusive.hash(state);
            }
            EvalOutput::Variant {
                type_name,
                variant_name,
                fields,
            } => {
                type_name.hash(state);
                variant_name.hash(state);
                fields.hash(state);
            }
        }
    }
}

/// Salsa-compatible snapshot of an `EvalError`'s diagnostic fields.
///
/// `EvalError` contains `Value` (not `Eq`/`Hash`) and `ControlFlow` (runtime-only),
/// so it cannot be stored directly in Salsa queries. This snapshot captures the
/// fields needed for diagnostic rendering: message, kind name, span, backtrace
/// frames, and notes.
///
/// Created at the Salsa query boundary via [`EvalErrorSnapshot::from_eval_error`].
///
/// # Salsa Compatibility
/// Has all required traits: Clone, Eq, `PartialEq`, Hash, Debug
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct EvalErrorSnapshot {
    /// Human-readable error message.
    pub message: String,
    /// Structured error kind name (e.g., `DivisionByZero`, `UndefinedVariable`).
    pub kind_name: String,
    /// The specific error code for this error kind (e.g., `E6001` for division by zero).
    ///
    /// Populated from `error_code_for_kind()` at snapshot creation time, ensuring
    /// the snapshot carries the exact same error code that `eval_error_to_diagnostic()`
    /// would produce. This avoids the lossy `kind_name` → error code reverse-mapping
    /// that `snapshot_to_diagnostic()` previously had to do (falling back to `E6099`).
    pub error_code: ErrorCode,
    /// Source location where the error occurred.
    pub span: Option<Span>,
    /// Call stack frames as `(function_name, optional_span)` pairs.
    pub backtrace: Vec<(String, Option<Span>)>,
    /// Additional context notes.
    pub notes: Vec<String>,
}

impl EvalErrorSnapshot {
    /// Create a snapshot from an `EvalError`, capturing diagnostic fields.
    ///
    /// Strips `Value` and `ControlFlow` (not Salsa-compatible) while preserving
    /// all information needed for diagnostic rendering.
    pub fn from_eval_error(err: &ori_patterns::EvalError) -> Self {
        let backtrace = err
            .backtrace
            .as_ref()
            .map(|bt| {
                bt.frames()
                    .iter()
                    .map(|frame| (frame.name.clone(), frame.span))
                    .collect()
            })
            .unwrap_or_default();

        let notes = err.notes.iter().map(|n| n.message.clone()).collect();

        let kind_name = err.kind.variant_name().to_string();

        let error_code = crate::problem::eval::error_code_for_kind(&err.kind);

        Self {
            message: err.message.clone(),
            kind_name,
            error_code,
            span: err.span,
            backtrace,
            notes,
        }
    }
}

/// Result of evaluating a module.
///
/// # Error Layering
///
/// This type uses a two-tier error design:
/// - `error` is the universal fallback: set for *any* failure (lex, parse, type,
///   runtime). Consumers that just need "did it fail?" check this field.
/// - `eval_error` is the structured runtime-only snapshot: set *only* when the
///   failure originated from the evaluator at runtime. It carries span, backtrace,
///   kind, and notes for enriched diagnostic rendering.
///
/// The `run` command checks `eval_error` first for rich diagnostics, then falls
/// back to `error` for pre-runtime failures (lex/parse/type errors).
///
/// # Salsa Compatibility
/// Has all required traits: Clone, Eq, `PartialEq`, Hash, Debug
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ModuleEvalResult {
    /// The result value (if evaluation succeeded).
    pub result: Option<EvalOutput>,
    /// Error message for any failure kind (lex, parse, type, or runtime).
    ///
    /// This is the universal "did it fail?" field. For runtime errors, the same
    /// message is also available in `eval_error.message` with richer context.
    pub error: Option<String>,
    /// Structured error snapshot, populated **only** for runtime eval errors.
    ///
    /// Preserves span, backtrace, notes, and kind information that the plain
    /// `error` field discards. Pre-runtime failures (lex, parse, type errors)
    /// leave this as `None` and use `error` alone.
    pub eval_error: Option<EvalErrorSnapshot>,
    /// Captured stdout output (if any).
    pub stdout: String,
}

impl ModuleEvalResult {
    /// Create a successful result.
    pub fn success(result: EvalOutput) -> Self {
        ModuleEvalResult {
            result: Some(result),
            error: None,
            eval_error: None,
            stdout: String::new(),
        }
    }

    /// Create an error result from a plain message (no structured error info).
    ///
    /// Used by [`crate::query::evaluated()`] to gate on upstream failures (lex,
    /// parse, type errors) without carrying structured diagnostics. Consumers
    /// that need rich error detail (spans, suggestions, error codes) should use
    /// `report_frontend_errors()` in `commands/mod.rs` instead, which queries
    /// each phase separately for full diagnostic quality.
    pub fn failure(error: String) -> Self {
        ModuleEvalResult {
            result: None,
            error: Some(error),
            eval_error: None,
            stdout: String::new(),
        }
    }

    /// Create an error result from an `EvalError`, preserving structured diagnostics.
    ///
    /// Captures the error's span, backtrace, notes, and kind into an
    /// [`EvalErrorSnapshot`] for enriched diagnostic rendering.
    pub fn runtime_error(err: &ori_patterns::EvalError) -> Self {
        ModuleEvalResult {
            result: None,
            error: Some(err.message.clone()),
            eval_error: Some(EvalErrorSnapshot::from_eval_error(err)),
            stdout: String::new(),
        }
    }

    /// Check if evaluation succeeded.
    pub fn is_success(&self) -> bool {
        self.error.is_none()
    }

    /// Check if evaluation failed.
    pub fn is_failure(&self) -> bool {
        self.error.is_some()
    }
}

impl Default for ModuleEvalResult {
    fn default() -> Self {
        ModuleEvalResult {
            result: Some(EvalOutput::Void),
            error: None,
            eval_error: None,
            stdout: String::new(),
        }
    }
}

#[cfg(test)]
#[expect(clippy::unwrap_used, reason = "Tests use unwrap for brevity")]
mod tests {
    use super::*;
    use crate::ir::SharedInterner;

    #[test]
    fn test_eval_output_from_value() {
        let interner = SharedInterner::default();

        assert_eq!(
            EvalOutput::from_value(&Value::int(42), &interner),
            EvalOutput::Int(42)
        );
        assert_eq!(
            EvalOutput::from_value(&Value::Bool(true), &interner),
            EvalOutput::Bool(true)
        );
        assert_eq!(
            EvalOutput::from_value(&Value::Void, &interner),
            EvalOutput::Void
        );
        assert_eq!(
            EvalOutput::from_value(&Value::None, &interner),
            EvalOutput::None
        );
    }

    #[test]
    fn test_eval_output_display() {
        let interner = SharedInterner::default();
        assert_eq!(EvalOutput::Int(42).display(&interner), "42");
        assert_eq!(EvalOutput::Bool(true).display(&interner), "true");
        assert_eq!(EvalOutput::Void.display(&interner), "void");
        assert_eq!(EvalOutput::None.display(&interner), "None");
        assert_eq!(
            EvalOutput::Some(Box::new(EvalOutput::Int(1))).display(&interner),
            "Some(1)"
        );
        assert_eq!(
            EvalOutput::Ok(Box::new(EvalOutput::Int(1))).display(&interner),
            "Ok(1)"
        );
        assert_eq!(
            EvalOutput::List(vec![EvalOutput::Int(1), EvalOutput::Int(2)]).display(&interner),
            "[1, 2]"
        );
        assert_eq!(
            EvalOutput::Tuple(vec![EvalOutput::Int(1), EvalOutput::Bool(true)]).display(&interner),
            "(1, true)"
        );
    }

    #[test]
    fn test_eval_output_variant_display() {
        let interner = SharedInterner::default();
        let type_name = interner.intern("Option");
        let some_name = interner.intern("Some");
        let none_name = interner.intern("None");

        // Variant without fields
        let none_variant = EvalOutput::Variant {
            type_name,
            variant_name: none_name,
            fields: vec![],
        };
        assert_eq!(none_variant.display(&interner), "Option::None");

        // Variant with fields
        let some_variant = EvalOutput::Variant {
            type_name,
            variant_name: some_name,
            fields: vec![EvalOutput::Int(42)],
        };
        assert_eq!(some_variant.display(&interner), "Option::Some(42)");
    }

    #[test]
    fn test_eval_output_equality() {
        assert_eq!(EvalOutput::Int(42), EvalOutput::Int(42));
        assert_ne!(EvalOutput::Int(42), EvalOutput::Int(43));
        assert_ne!(EvalOutput::Int(42), EvalOutput::Bool(true));

        assert_eq!(
            EvalOutput::List(vec![EvalOutput::Int(1)]),
            EvalOutput::List(vec![EvalOutput::Int(1)])
        );
    }

    #[test]
    fn test_module_eval_result() {
        let success = ModuleEvalResult::success(EvalOutput::Int(42));
        assert!(success.is_success());
        assert!(!success.is_failure());
        assert!(success.eval_error.is_none());

        let failure = ModuleEvalResult::failure("test error".to_string());
        assert!(!failure.is_success());
        assert!(failure.is_failure());
        assert!(failure.eval_error.is_none());
    }

    #[test]
    fn test_runtime_error_preserves_snapshot() {
        let err = ori_patterns::division_by_zero().with_span(Span::new(10, 20));
        let result = ModuleEvalResult::runtime_error(&err);

        assert!(result.is_failure());
        assert!(result.error.as_ref().unwrap().contains("division by zero"));

        let snapshot = result.eval_error.as_ref().unwrap();
        assert_eq!(snapshot.span, Some(Span::new(10, 20)));
        assert_eq!(snapshot.kind_name, "DivisionByZero");
        assert_eq!(snapshot.error_code, ErrorCode::E6001);
        assert!(snapshot.message.contains("division by zero"));
    }

    #[test]
    fn test_snapshot_captures_backtrace() {
        use ori_patterns::{BacktraceFrame, EvalBacktrace};

        let bt = EvalBacktrace::new(vec![
            BacktraceFrame {
                name: "foo".to_string(),
                span: Some(Span::new(5, 10)),
            },
            BacktraceFrame {
                name: "bar".to_string(),
                span: None,
            },
        ]);
        let err = ori_patterns::division_by_zero().with_backtrace(bt);
        let snapshot = EvalErrorSnapshot::from_eval_error(&err);

        assert_eq!(snapshot.backtrace.len(), 2);
        assert_eq!(snapshot.backtrace[0].0, "foo");
        assert_eq!(snapshot.backtrace[0].1, Some(Span::new(5, 10)));
        assert_eq!(snapshot.backtrace[1].0, "bar");
        assert_eq!(snapshot.backtrace[1].1, None);
    }

    #[test]
    fn test_snapshot_captures_notes() {
        use ori_patterns::EvalNote;

        let err = ori_patterns::division_by_zero()
            .with_note(EvalNote {
                message: "check denominator".to_string(),
                span: None,
            })
            .with_note(EvalNote {
                message: "second note".to_string(),
                span: Some(Span::new(0, 5)),
            });
        let snapshot = EvalErrorSnapshot::from_eval_error(&err);

        assert_eq!(snapshot.notes.len(), 2);
        assert_eq!(snapshot.notes[0], "check denominator");
        assert_eq!(snapshot.notes[1], "second note");
    }

    #[test]
    fn test_snapshot_salsa_traits() {
        // Verify Clone + Eq + Hash work (required for Salsa)
        use std::collections::HashSet;

        let err = ori_patterns::division_by_zero().with_span(Span::new(0, 5));
        let snapshot = EvalErrorSnapshot::from_eval_error(&err);
        let cloned = snapshot.clone();
        assert_eq!(snapshot, cloned);

        let mut set = HashSet::new();
        set.insert(snapshot.clone());
        assert!(set.contains(&cloned));
    }
}
