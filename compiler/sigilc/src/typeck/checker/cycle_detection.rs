//! Cycle Detection
//!
//! Detects closure self-capture and collects free variables from expressions.

use std::collections::HashSet;
use crate::ir::{Name, Span, ExprId, ExprKind, BindingPattern};
use super::TypeChecker;
use super::types::TypeCheckError;
use super::super::infer;

impl<'a> TypeChecker<'a> {
    /// Collect free variable references from an expression.
    ///
    /// This is used for closure self-capture detection. A variable is "free"
    /// if it's referenced but not bound within the expression.
    pub(crate) fn collect_free_vars(&self, expr_id: ExprId, bound: &HashSet<Name>) -> HashSet<Name> {
        let mut free = HashSet::new();
        infer::collect_free_vars_inner(self, expr_id, bound, &mut free);
        free
    }

    /// Add bindings from a pattern to the bound set.
    pub(crate) fn add_pattern_bindings(&self, pattern: &BindingPattern, bound: &mut HashSet<Name>) {
        match pattern {
            BindingPattern::Name(name) => {
                bound.insert(*name);
            }
            BindingPattern::Wildcard => {}
            BindingPattern::Tuple(patterns) => {
                for p in patterns {
                    self.add_pattern_bindings(p, bound);
                }
            }
            BindingPattern::Struct { fields } => {
                for (field_name, nested) in fields {
                    if let Some(nested_pattern) = nested {
                        self.add_pattern_bindings(nested_pattern, bound);
                    } else {
                        // Shorthand: { x } binds x
                        bound.insert(*field_name);
                    }
                }
            }
            BindingPattern::List { elements, rest } => {
                for p in elements {
                    self.add_pattern_bindings(p, bound);
                }
                if let Some(rest_name) = rest {
                    bound.insert(*rest_name);
                }
            }
        }
    }

    /// Check for closure self-capture in a let binding.
    ///
    /// Detects patterns like: `let f = () -> f()` where a closure captures itself.
    /// This would create a reference cycle and must be rejected at compile time.
    pub(crate) fn check_closure_self_capture(
        &mut self,
        pattern: &BindingPattern,
        init: ExprId,
        span: Span,
    ) {
        // Get the names being bound
        let mut bound_names = HashSet::new();
        self.add_pattern_bindings(pattern, &mut bound_names);

        // Check if init is a lambda that references any of the bound names
        let expr = self.arena.get_expr(init);
        if let ExprKind::Lambda { body, params, .. } = &expr.kind {
            // The lambda's parameters are bound in its body
            let mut lambda_bound = HashSet::new();
            for param in self.arena.get_params(*params) {
                lambda_bound.insert(param.name);
            }

            // Collect free variables from the lambda body
            let free_vars = self.collect_free_vars(*body, &lambda_bound);

            // Check if any bound name is in the free variables
            for name in &bound_names {
                if free_vars.contains(name) {
                    let name_str = self.interner.lookup(*name);
                    self.errors.push(TypeCheckError {
                        message: format!(
                            "closure cannot capture itself: `{}` references itself in its body",
                            name_str
                        ),
                        span,
                        code: crate::diagnostic::ErrorCode::E2007,
                    });
                }
            }
        }
    }
}
