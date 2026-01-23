//! Core function_exp pattern implementations.
//!
//! These patterns provide core functionality like assertions, length checking,
//! printing, etc. They use named argument syntax like all other function_exp patterns.

use crate::types::Type;
use crate::eval::{Value, EvalResult, EvalError};
use super::{PatternDefinition, TypeCheckContext, EvalContext, PatternExecutor};

// =============================================================================
// Assert Pattern
// =============================================================================

/// The `assert` pattern asserts a condition is true.
///
/// Syntax: `assert(.cond: expr)`
/// Type: `assert(.cond: bool) -> void`
pub struct AssertPattern;

impl PatternDefinition for AssertPattern {
    fn name(&self) -> &'static str {
        "assert"
    }

    fn required_props(&self) -> &'static [&'static str] {
        &["cond"]
    }

    fn type_check(&self, _ctx: &mut TypeCheckContext) -> Type {
        Type::Unit
    }

    fn evaluate(&self, ctx: &EvalContext, exec: &mut dyn PatternExecutor) -> EvalResult {
        let cond = ctx.eval_prop("cond", exec)?;
        if cond.is_truthy() {
            Ok(Value::Void)
        } else {
            Err(EvalError::new("assertion failed"))
        }
    }
}

// =============================================================================
// AssertEq Pattern
// =============================================================================

/// The `assert_eq` pattern asserts two values are equal.
///
/// Syntax: `assert_eq(.actual: expr, .expected: expr)`
/// Type: `assert_eq(.actual: T, .expected: T) -> void`
pub struct AssertEqPattern;

impl PatternDefinition for AssertEqPattern {
    fn name(&self) -> &'static str {
        "assert_eq"
    }

    fn required_props(&self) -> &'static [&'static str] {
        &["actual", "expected"]
    }

    fn type_check(&self, _ctx: &mut TypeCheckContext) -> Type {
        Type::Unit
    }

    fn evaluate(&self, ctx: &EvalContext, exec: &mut dyn PatternExecutor) -> EvalResult {
        let actual = ctx.eval_prop("actual", exec)?;
        let expected = ctx.eval_prop("expected", exec)?;
        if actual.equals(&expected) {
            Ok(Value::Void)
        } else {
            Err(EvalError::new(format!(
                "assertion failed: {} != {}",
                actual.display_value(),
                expected.display_value()
            )))
        }
    }
}

// =============================================================================
// AssertNe Pattern
// =============================================================================

/// The `assert_ne` pattern asserts two values are not equal.
///
/// Syntax: `assert_ne(.actual: expr, .unexpected: expr)`
/// Type: `assert_ne(.actual: T, .unexpected: T) -> void`
pub struct AssertNePattern;

impl PatternDefinition for AssertNePattern {
    fn name(&self) -> &'static str {
        "assert_ne"
    }

    fn required_props(&self) -> &'static [&'static str] {
        &["actual", "unexpected"]
    }

    fn type_check(&self, _ctx: &mut TypeCheckContext) -> Type {
        Type::Unit
    }

    fn evaluate(&self, ctx: &EvalContext, exec: &mut dyn PatternExecutor) -> EvalResult {
        let actual = ctx.eval_prop("actual", exec)?;
        let unexpected = ctx.eval_prop("unexpected", exec)?;
        if !actual.equals(&unexpected) {
            Ok(Value::Void)
        } else {
            Err(EvalError::new(format!(
                "assertion failed: values are equal: {}",
                actual.display_value()
            )))
        }
    }
}

// =============================================================================
// Len Pattern
// =============================================================================

/// The `len` pattern returns the length of a collection.
///
/// Syntax: `len(.collection: expr)`
/// Type: `len(.collection: [T] | str | {K: V}) -> int`
pub struct LenPattern;

impl PatternDefinition for LenPattern {
    fn name(&self) -> &'static str {
        "len"
    }

    fn required_props(&self) -> &'static [&'static str] {
        &["collection"]
    }

    fn type_check(&self, _ctx: &mut TypeCheckContext) -> Type {
        Type::Int
    }

    fn evaluate(&self, ctx: &EvalContext, exec: &mut dyn PatternExecutor) -> EvalResult {
        let collection = ctx.eval_prop("collection", exec)?;
        match &collection {
            Value::List(items) => Ok(Value::Int(items.len() as i64)),
            Value::Str(s) => Ok(Value::Int(s.chars().count() as i64)),
            Value::Map(m) => Ok(Value::Int(m.len() as i64)),
            Value::Tuple(items) => Ok(Value::Int(items.len() as i64)),
            _ => Err(EvalError::new(format!(
                "len requires a collection, got {}",
                collection.type_name()
            ))),
        }
    }
}

// =============================================================================
// IsEmpty Pattern
// =============================================================================

/// The `is_empty` pattern checks if a collection is empty.
///
/// Syntax: `is_empty(.collection: expr)`
/// Type: `is_empty(.collection: [T] | str | {K: V}) -> bool`
pub struct IsEmptyPattern;

impl PatternDefinition for IsEmptyPattern {
    fn name(&self) -> &'static str {
        "is_empty"
    }

    fn required_props(&self) -> &'static [&'static str] {
        &["collection"]
    }

    fn type_check(&self, _ctx: &mut TypeCheckContext) -> Type {
        Type::Bool
    }

    fn evaluate(&self, ctx: &EvalContext, exec: &mut dyn PatternExecutor) -> EvalResult {
        let collection = ctx.eval_prop("collection", exec)?;
        let is_empty = match &collection {
            Value::List(items) => items.is_empty(),
            Value::Str(s) => s.is_empty(),
            Value::Map(m) => m.is_empty(),
            Value::Tuple(items) => items.is_empty(),
            _ => return Err(EvalError::new(format!(
                "is_empty requires a collection, got {}",
                collection.type_name()
            ))),
        };
        Ok(Value::Bool(is_empty))
    }
}

// =============================================================================
// IsSome Pattern
// =============================================================================

/// The `is_some` pattern checks if an Option is Some.
///
/// Syntax: `is_some(.opt: expr)`
/// Type: `is_some(.opt: Option<T>) -> bool`
pub struct IsSomePattern;

impl PatternDefinition for IsSomePattern {
    fn name(&self) -> &'static str {
        "is_some"
    }

    fn required_props(&self) -> &'static [&'static str] {
        &["opt"]
    }

    fn type_check(&self, _ctx: &mut TypeCheckContext) -> Type {
        Type::Bool
    }

    fn evaluate(&self, ctx: &EvalContext, exec: &mut dyn PatternExecutor) -> EvalResult {
        let opt = ctx.eval_prop("opt", exec)?;
        Ok(Value::Bool(matches!(opt, Value::Some(_))))
    }
}

// =============================================================================
// IsNone Pattern
// =============================================================================

/// The `is_none` pattern checks if an Option is None.
///
/// Syntax: `is_none(.opt: expr)`
/// Type: `is_none(.opt: Option<T>) -> bool`
pub struct IsNonePattern;

impl PatternDefinition for IsNonePattern {
    fn name(&self) -> &'static str {
        "is_none"
    }

    fn required_props(&self) -> &'static [&'static str] {
        &["opt"]
    }

    fn type_check(&self, _ctx: &mut TypeCheckContext) -> Type {
        Type::Bool
    }

    fn evaluate(&self, ctx: &EvalContext, exec: &mut dyn PatternExecutor) -> EvalResult {
        let opt = ctx.eval_prop("opt", exec)?;
        Ok(Value::Bool(matches!(opt, Value::None)))
    }
}

// =============================================================================
// IsOk Pattern
// =============================================================================

/// The `is_ok` pattern checks if a Result is Ok.
///
/// Syntax: `is_ok(.result: expr)`
/// Type: `is_ok(.result: Result<T, E>) -> bool`
pub struct IsOkPattern;

impl PatternDefinition for IsOkPattern {
    fn name(&self) -> &'static str {
        "is_ok"
    }

    fn required_props(&self) -> &'static [&'static str] {
        &["result"]
    }

    fn type_check(&self, _ctx: &mut TypeCheckContext) -> Type {
        Type::Bool
    }

    fn evaluate(&self, ctx: &EvalContext, exec: &mut dyn PatternExecutor) -> EvalResult {
        let result = ctx.eval_prop("result", exec)?;
        Ok(Value::Bool(matches!(result, Value::Ok(_))))
    }
}

// =============================================================================
// IsErr Pattern
// =============================================================================

/// The `is_err` pattern checks if a Result is Err.
///
/// Syntax: `is_err(.result: expr)`
/// Type: `is_err(.result: Result<T, E>) -> bool`
pub struct IsErrPattern;

impl PatternDefinition for IsErrPattern {
    fn name(&self) -> &'static str {
        "is_err"
    }

    fn required_props(&self) -> &'static [&'static str] {
        &["result"]
    }

    fn type_check(&self, _ctx: &mut TypeCheckContext) -> Type {
        Type::Bool
    }

    fn evaluate(&self, ctx: &EvalContext, exec: &mut dyn PatternExecutor) -> EvalResult {
        let result = ctx.eval_prop("result", exec)?;
        Ok(Value::Bool(matches!(result, Value::Err(_))))
    }
}

// =============================================================================
// Print Pattern
// =============================================================================

/// The `print` pattern prints a message to stdout.
///
/// Syntax: `print(.msg: expr)`
/// Type: `print(.msg: str) -> void`
pub struct PrintPattern;

impl PatternDefinition for PrintPattern {
    fn name(&self) -> &'static str {
        "print"
    }

    fn required_props(&self) -> &'static [&'static str] {
        &["msg"]
    }

    fn type_check(&self, _ctx: &mut TypeCheckContext) -> Type {
        Type::Unit
    }

    fn evaluate(&self, ctx: &EvalContext, exec: &mut dyn PatternExecutor) -> EvalResult {
        let msg = ctx.eval_prop("msg", exec)?;
        println!("{}", msg.display_value());
        Ok(Value::Void)
    }
}

// =============================================================================
// Panic Pattern
// =============================================================================

/// The `panic` pattern halts execution with an error message.
///
/// Syntax: `panic(.msg: expr)`
/// Type: `panic(.msg: str) -> Never`
pub struct PanicPattern;

impl PatternDefinition for PanicPattern {
    fn name(&self) -> &'static str {
        "panic"
    }

    fn required_props(&self) -> &'static [&'static str] {
        &["msg"]
    }

    fn type_check(&self, _ctx: &mut TypeCheckContext) -> Type {
        Type::Never
    }

    fn evaluate(&self, ctx: &EvalContext, exec: &mut dyn PatternExecutor) -> EvalResult {
        let msg = ctx.eval_prop("msg", exec)?;
        Err(EvalError::new(format!("panic: {}", msg.display_value())))
    }
}

// =============================================================================
// Compare Pattern
// =============================================================================

/// The `compare` pattern compares two values and returns an Ordering.
///
/// Syntax: `compare(.left: expr, .right: expr)`
/// Type: `compare(.left: T, .right: T) -> Ordering`
pub struct ComparePattern;

impl PatternDefinition for ComparePattern {
    fn name(&self) -> &'static str {
        "compare"
    }

    fn required_props(&self) -> &'static [&'static str] {
        &["left", "right"]
    }

    fn type_check(&self, _ctx: &mut TypeCheckContext) -> Type {
        // Ordering is a sum type, for now return a placeholder
        Type::Int // TODO: Return proper Ordering type when sum types are implemented
    }

    fn evaluate(&self, ctx: &EvalContext, exec: &mut dyn PatternExecutor) -> EvalResult {
        let left = ctx.eval_prop("left", exec)?;
        let right = ctx.eval_prop("right", exec)?;

        let ordering = match (&left, &right) {
            (Value::Int(a), Value::Int(b)) => a.cmp(b),
            (Value::Float(a), Value::Float(b)) => a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal),
            (Value::Str(a), Value::Str(b)) => a.cmp(b),
            _ => return Err(EvalError::new(format!(
                "compare requires comparable types, got {} and {}",
                left.type_name(),
                right.type_name()
            ))),
        };

        // Return as int: -1 = Less, 0 = Equal, 1 = Greater
        Ok(Value::Int(match ordering {
            std::cmp::Ordering::Less => -1,
            std::cmp::Ordering::Equal => 0,
            std::cmp::Ordering::Greater => 1,
        }))
    }
}

// =============================================================================
// Min Pattern
// =============================================================================

/// The `min` pattern returns the smaller of two values.
///
/// Syntax: `min(.left: expr, .right: expr)`
/// Type: `min(.left: T, .right: T) -> T`
pub struct MinPattern;

impl PatternDefinition for MinPattern {
    fn name(&self) -> &'static str {
        "min"
    }

    fn required_props(&self) -> &'static [&'static str] {
        &["left", "right"]
    }

    fn type_check(&self, ctx: &mut TypeCheckContext) -> Type {
        // Return type is same as input type
        ctx.require_prop_type("left")
    }

    fn evaluate(&self, ctx: &EvalContext, exec: &mut dyn PatternExecutor) -> EvalResult {
        let left = ctx.eval_prop("left", exec)?;
        let right = ctx.eval_prop("right", exec)?;

        let result = match (&left, &right) {
            (Value::Int(a), Value::Int(b)) => if a <= b { left } else { right },
            (Value::Float(a), Value::Float(b)) => if a <= b { left } else { right },
            _ => return Err(EvalError::new(format!(
                "min requires comparable types, got {} and {}",
                left.type_name(),
                right.type_name()
            ))),
        };

        Ok(result)
    }
}

// =============================================================================
// Max Pattern
// =============================================================================

/// The `max` pattern returns the larger of two values.
///
/// Syntax: `max(.left: expr, .right: expr)`
/// Type: `max(.left: T, .right: T) -> T`
pub struct MaxPattern;

impl PatternDefinition for MaxPattern {
    fn name(&self) -> &'static str {
        "max"
    }

    fn required_props(&self) -> &'static [&'static str] {
        &["left", "right"]
    }

    fn type_check(&self, ctx: &mut TypeCheckContext) -> Type {
        // Return type is same as input type
        ctx.require_prop_type("left")
    }

    fn evaluate(&self, ctx: &EvalContext, exec: &mut dyn PatternExecutor) -> EvalResult {
        let left = ctx.eval_prop("left", exec)?;
        let right = ctx.eval_prop("right", exec)?;

        let result = match (&left, &right) {
            (Value::Int(a), Value::Int(b)) => if a >= b { left } else { right },
            (Value::Float(a), Value::Float(b)) => if a >= b { left } else { right },
            _ => return Err(EvalError::new(format!(
                "max requires comparable types, got {} and {}",
                left.type_name(),
                right.type_name()
            ))),
        };

        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pattern_names() {
        assert_eq!(AssertPattern.name(), "assert");
        assert_eq!(AssertEqPattern.name(), "assert_eq");
        assert_eq!(AssertNePattern.name(), "assert_ne");
        assert_eq!(LenPattern.name(), "len");
        assert_eq!(IsEmptyPattern.name(), "is_empty");
        assert_eq!(IsSomePattern.name(), "is_some");
        assert_eq!(IsNonePattern.name(), "is_none");
        assert_eq!(IsOkPattern.name(), "is_ok");
        assert_eq!(IsErrPattern.name(), "is_err");
        assert_eq!(PrintPattern.name(), "print");
        assert_eq!(PanicPattern.name(), "panic");
        assert_eq!(ComparePattern.name(), "compare");
        assert_eq!(MinPattern.name(), "min");
        assert_eq!(MaxPattern.name(), "max");
    }

    #[test]
    fn test_required_props() {
        assert_eq!(AssertPattern.required_props(), &["cond"]);
        assert_eq!(AssertEqPattern.required_props(), &["actual", "expected"]);
        assert_eq!(LenPattern.required_props(), &["collection"]);
        assert_eq!(MinPattern.required_props(), &["left", "right"]);
    }
}
