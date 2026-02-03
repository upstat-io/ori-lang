//! Cycle Detection
//!
//! Detects closure self-capture and collects free variables from expressions.

use super::TypeChecker;
use crate::infer;
use ori_ir::{BindingPattern, ExprId, ExprKind, Name, Span};
use std::collections::HashSet;

/// Add bindings from a pattern to the bound set.
#[expect(clippy::implicit_hasher, reason = "Standard HashSet sufficient here")]
pub fn add_pattern_bindings(pattern: &BindingPattern, bound: &mut HashSet<Name>) {
    match pattern {
        BindingPattern::Name(name) => {
            bound.insert(*name);
        }
        BindingPattern::Tuple(patterns) => {
            for p in patterns {
                add_pattern_bindings(p, bound);
            }
        }
        BindingPattern::Struct { fields } => {
            for (field_name, opt_pattern) in fields {
                match opt_pattern {
                    Some(nested) => add_pattern_bindings(nested, bound),
                    None => {
                        bound.insert(*field_name);
                    }
                }
            }
        }
        BindingPattern::List { elements, rest } => {
            for p in elements {
                add_pattern_bindings(p, bound);
            }
            if let Some(rest_name) = rest {
                bound.insert(*rest_name);
            }
        }
        BindingPattern::Wildcard => {}
    }
}

impl TypeChecker<'_> {
    /// Collect free variable references from an expression.
    ///
    /// This is used for closure self-capture detection. A variable is "free"
    /// if it's referenced but not bound within the expression.
    pub(crate) fn collect_free_vars(
        &self,
        expr_id: ExprId,
        bound: &HashSet<Name>,
    ) -> HashSet<Name> {
        let mut free = HashSet::new();
        infer::collect_free_vars_inner(self, expr_id, bound, &mut free);
        free
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
        add_pattern_bindings(pattern, &mut bound_names);

        // Check if init is a lambda that references any of the bound names
        let expr = self.context.arena.get_expr(init);
        if let ExprKind::Lambda { body, params, .. } = &expr.kind {
            // The lambda's parameters are bound in its body
            let mut lambda_bound = HashSet::new();
            for param in self.context.arena.get_params(*params) {
                lambda_bound.insert(param.name);
            }

            // Collect free variables from the lambda body
            let free_vars = self.collect_free_vars(*body, &lambda_bound);

            // Check if any bound name is in the free variables
            for name in &bound_names {
                if free_vars.contains(name) {
                    let name_str = self.context.interner.lookup(*name);
                    self.error_closure_self_capture(span, name_str);
                }
            }
        }
    }
}
