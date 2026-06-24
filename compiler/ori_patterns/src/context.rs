//! Pattern evaluation context: [`EvalContext`].

use ori_ir::{ExprArena, ExprId, StringInterner};

use crate::errors::{ControlAction, EvalError, EvalResult};
use crate::executor::PatternExecutor;
use crate::value::Value;
use ori_ir::NamedExpr;

/// Context for evaluating a pattern.
///
/// Provides access to the evaluator's components without exposing the full evaluator.
pub struct EvalContext<'a> {
    pub interner: &'a StringInterner,
    pub arena: &'a ExprArena,
    /// Named expressions (properties) for this pattern.
    pub props: &'a [NamedExpr],
}

impl<'a> EvalContext<'a> {
    /// Create a new evaluation context.
    pub fn new(interner: &'a StringInterner, arena: &'a ExprArena, props: &'a [NamedExpr]) -> Self {
        EvalContext {
            interner,
            arena,
            props,
        }
    }

    /// Get the span of the first property, if any.
    ///
    /// Used as a fallback span for errors when no specific property is available.
    pub fn first_prop_span(&self) -> Option<ori_ir::Span> {
        self.props
            .first()
            .map(|p| self.arena.get_expr(p.value).span)
    }

    /// Get the span of a named property, if present.
    pub fn prop_span(&self, name: &str) -> Option<ori_ir::Span> {
        let target = self.interner.intern(name);
        for prop in self.props {
            if prop.name == target {
                return Some(self.arena.get_expr(prop.value).span);
            }
        }
        None
    }

    /// Get a required property's `ExprId` by name.
    #[allow(
        clippy::result_large_err,
        reason = "EvalError is fundamental — boxing would add complexity across the crate"
    )]
    pub fn get_prop(&self, name: &str) -> Result<ExprId, EvalError> {
        let target = self.interner.intern(name);
        for prop in self.props {
            if prop.name == target {
                return Ok(prop.value);
            }
        }
        let mut err = EvalError::new(format!("missing required property: .{name}"));
        if let Some(span) = self.first_prop_span() {
            err = err.with_span(span);
        }
        Err(err)
    }

    /// Get an optional property's `ExprId` by name.
    pub fn get_prop_opt(&self, name: &str) -> Option<ExprId> {
        let target = self.interner.intern(name);
        for prop in self.props {
            if prop.name == target {
                return Some(prop.value);
            }
        }
        None
    }

    /// Get a required property and evaluate it.
    ///
    /// This is a convenience method that combines `get_prop` and `exec.eval`.
    pub fn eval_prop(&self, name: &str, exec: &mut dyn PatternExecutor) -> EvalResult {
        let expr_id = self.get_prop(name)?;
        exec.eval(expr_id)
    }

    /// Get a required property and evaluate it, attaching span on error.
    ///
    /// This is like `eval_prop` but attaches the property's span to any evaluation error,
    /// providing better error messages with location information.
    pub fn eval_prop_spanned(&self, name: &str, exec: &mut dyn PatternExecutor) -> EvalResult {
        let expr_id = self.get_prop(name)?;
        let span = self.arena.get_expr(expr_id).span;
        exec.eval(expr_id)
            .map_err(|action| action.with_span_if_error(span))
    }

    /// Get an optional property and evaluate it if present.
    ///
    /// Returns `Ok(None)` if the property is not present, `Ok(Some(value))` if present
    /// and evaluation succeeds, or `Err` if evaluation fails.
    pub fn eval_prop_opt(
        &self,
        name: &str,
        exec: &mut dyn PatternExecutor,
    ) -> Result<Option<Value>, ControlAction> {
        match self.get_prop_opt(name) {
            Some(expr_id) => Ok(Some(exec.eval(expr_id)?)),
            None => Ok(None),
        }
    }

    /// Get an optional property and evaluate it if present, attaching span on error.
    ///
    /// This is like `eval_prop_opt` but attaches the property's span to any evaluation error.
    pub fn eval_prop_opt_spanned(
        &self,
        name: &str,
        exec: &mut dyn PatternExecutor,
    ) -> Result<Option<Value>, ControlAction> {
        match self.get_prop_opt(name) {
            Some(expr_id) => {
                let span = self.arena.get_expr(expr_id).span;
                let value = exec
                    .eval(expr_id)
                    .map_err(|action| action.with_span_if_error(span))?;
                Ok(Some(value))
            }
            None => Ok(None),
        }
    }

    /// Create an error with span attached from a named property.
    ///
    /// If the property exists, attaches its span to the error.
    /// Otherwise, uses the first property's span as a fallback.
    #[cold]
    pub fn error_with_prop_span(&self, message: impl Into<String>, prop_name: &str) -> EvalError {
        let err = EvalError::new(message);
        if let Some(span) = self.prop_span(prop_name) {
            err.with_span(span)
        } else if let Some(span) = self.first_prop_span() {
            err.with_span(span)
        } else {
            err
        }
    }
}
