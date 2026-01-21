// Lambda capture analysis for Sigil
// Determines which variables from outer scopes are captured by lambdas

use crate::ast::Expr;
use crate::ir::LocalId;
use std::collections::{HashMap, HashSet};

/// Analyze captures for a lambda expression
pub struct CaptureAnalyzer {
    /// Lambda parameters (not captures)
    params: HashSet<String>,
    /// Locally defined variables within the lambda
    locals: HashSet<String>,
    /// Variables accessed that need to be captured
    accessed: HashSet<String>,
}

impl CaptureAnalyzer {
    pub fn new() -> Self {
        CaptureAnalyzer {
            params: HashSet::new(),
            locals: HashSet::new(),
            accessed: HashSet::new(),
        }
    }

    /// Analyze a lambda body and return the names of captured variables
    ///
    /// # Arguments
    /// * `params` - Names of lambda parameters
    /// * `body` - The lambda body expression
    ///
    /// # Returns
    /// Set of variable names that are free (need to be captured)
    pub fn analyze(&mut self, params: &[String], body: &Expr) -> HashSet<String> {
        // Register lambda parameters
        for param in params {
            self.params.insert(param.clone());
        }

        // Walk the body to find accessed variables
        self.visit_expr(body);

        // Free variables = accessed - params - locals
        self.accessed
            .difference(&self.params)
            .filter(|name| !self.locals.contains(*name))
            .cloned()
            .collect()
    }

    /// Walk an expression to find variable accesses
    fn visit_expr(&mut self, expr: &Expr) {
        match expr {
            // Variable access - record it
            Expr::Ident(name) => {
                self.accessed.insert(name.clone());
            }

            // Let binding - adds to locals
            Expr::Let { name, value, .. } => {
                self.visit_expr(value);
                self.locals.insert(name.clone());
            }

            // Reassignment - both access and visit value
            Expr::Reassign { target, value } => {
                self.accessed.insert(target.clone());
                self.visit_expr(value);
            }

            // Nested lambda - creates a new scope, don't recurse into it
            // (nested lambdas will have their own capture analysis)
            Expr::Lambda { .. } => {
                // Don't recurse - nested lambda has its own scope
            }

            // Binary operations
            Expr::Binary { left, right, .. } => {
                self.visit_expr(left);
                self.visit_expr(right);
            }

            // Unary operations
            Expr::Unary { operand, .. } => {
                self.visit_expr(operand);
            }

            // Function call
            Expr::Call { func, args } => {
                self.visit_expr(func);
                for arg in args {
                    self.visit_expr(arg);
                }
            }

            // Method call
            Expr::MethodCall { receiver, args, .. } => {
                self.visit_expr(receiver);
                for arg in args {
                    self.visit_expr(arg);
                }
            }

            // Field access
            Expr::Field(obj, _) => {
                self.visit_expr(obj);
            }

            // Index access
            Expr::Index(obj, idx) => {
                self.visit_expr(obj);
                self.visit_expr(idx);
            }

            // If expression
            Expr::If { condition, then_branch, else_branch } => {
                self.visit_expr(condition);
                self.visit_expr(then_branch);
                if let Some(else_br) = else_branch {
                    self.visit_expr(else_br);
                }
            }

            // Match expression
            Expr::Match(m) => {
                self.visit_expr(&m.scrutinee);
                for arm in &m.arms {
                    // Pattern variables are local to the arm
                    self.visit_expr(&arm.body);
                }
            }

            // Block expression
            Expr::Block(exprs) => {
                for expr in exprs {
                    self.visit_expr(expr);
                }
            }

            // For loop
            Expr::For { binding, iterator, body } => {
                self.visit_expr(iterator);
                // The binding is local to the loop
                let old_locals = self.locals.clone();
                self.locals.insert(binding.clone());
                self.visit_expr(body);
                self.locals = old_locals;
            }

            // Range
            Expr::Range { start, end } => {
                self.visit_expr(start);
                self.visit_expr(end);
            }

            // Collections
            Expr::List(exprs) => {
                for expr in exprs {
                    self.visit_expr(expr);
                }
            }

            Expr::Tuple(exprs) => {
                for expr in exprs {
                    self.visit_expr(expr);
                }
            }

            Expr::MapLiteral(entries) => {
                for (k, v) in entries {
                    self.visit_expr(k);
                    self.visit_expr(v);
                }
            }

            Expr::Struct { fields, .. } => {
                for (_, expr) in fields {
                    self.visit_expr(expr);
                }
            }

            // Result/Option wrappers
            Expr::Ok(inner) | Expr::Err(inner) | Expr::Some(inner) | Expr::Unwrap(inner) => {
                self.visit_expr(inner);
            }

            // Coalesce
            Expr::Coalesce { value, default } => {
                self.visit_expr(value);
                self.visit_expr(default);
            }

            // Pattern expressions
            Expr::Pattern(p) => {
                self.visit_pattern_expr(p);
            }

            // Config variables are not captures (they're global)
            Expr::Config(_) => {}

            // Literals don't access variables
            Expr::Int(_) | Expr::Float(_) | Expr::String(_) | Expr::Bool(_) | Expr::Nil => {}

            // None is a literal
            Expr::None_ => {}

            // Length placeholder
            Expr::LengthPlaceholder => {}
        }
    }

    /// Visit a pattern expression
    fn visit_pattern_expr(&mut self, pattern: &crate::ast::PatternExpr) {
        use crate::ast::PatternExpr;

        match pattern {
            PatternExpr::Fold { collection, initial, reducer, .. } => {
                self.visit_expr(collection);
                self.visit_expr(initial);
                self.visit_expr(reducer);
            }
            PatternExpr::Map { collection, transform, .. } => {
                self.visit_expr(collection);
                self.visit_expr(transform);
            }
            PatternExpr::Filter { collection, predicate, .. } => {
                self.visit_expr(collection);
                self.visit_expr(predicate);
            }
            PatternExpr::Collect { range, transform, .. } => {
                self.visit_expr(range);
                self.visit_expr(transform);
            }
            PatternExpr::Recurse { condition, base_case, recursive_case, .. } => {
                self.visit_expr(condition);
                self.visit_expr(base_case);
                self.visit_expr(recursive_case);
            }
            PatternExpr::Run { steps } => {
                for step in steps {
                    self.visit_expr(step);
                }
            }
            PatternExpr::Match { scrutinee, arms, .. } => {
                self.visit_expr(scrutinee);
                for (_, body) in arms {
                    self.visit_expr(body);
                }
            }
            PatternExpr::Parallel { tasks } => {
                for (_, expr) in tasks {
                    self.visit_expr(expr);
                }
            }
            PatternExpr::Chain { initial, steps } => {
                self.visit_expr(initial);
                for step in steps {
                    self.visit_expr(step);
                }
            }
        }
    }
}

/// Resolve captured variable names to LocalIds
pub fn resolve_captures(
    free_vars: &HashSet<String>,
    outer_scope: &HashMap<String, LocalId>,
) -> Vec<LocalId> {
    free_vars
        .iter()
        .filter_map(|name| outer_scope.get(name).copied())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::BinaryOp;

    #[test]
    fn test_no_captures() {
        let mut analyzer = CaptureAnalyzer::new();
        let body = Expr::Ident("x".to_string());
        let captures = analyzer.analyze(&["x".to_string()], &body);
        assert!(captures.is_empty());
    }

    #[test]
    fn test_single_capture() {
        let mut analyzer = CaptureAnalyzer::new();
        let body = Expr::Binary {
            op: BinaryOp::Add,
            left: Box::new(Expr::Ident("x".to_string())),
            right: Box::new(Expr::Ident("y".to_string())),
        };
        let captures = analyzer.analyze(&["x".to_string()], &body);
        assert_eq!(captures.len(), 1);
        assert!(captures.contains("y"));
    }

    #[test]
    fn test_local_not_captured() {
        let mut analyzer = CaptureAnalyzer::new();
        let body = Expr::Block(vec![
            Expr::Let {
                name: "y".to_string(),
                mutable: false,
                value: Box::new(Expr::Int(1)),
            },
            Expr::Ident("y".to_string()),
        ]);
        let captures = analyzer.analyze(&["x".to_string()], &body);
        assert!(captures.is_empty());
    }

    #[test]
    fn test_multiple_captures() {
        let mut analyzer = CaptureAnalyzer::new();
        let body = Expr::Binary {
            op: BinaryOp::Add,
            left: Box::new(Expr::Ident("a".to_string())),
            right: Box::new(Expr::Binary {
                op: BinaryOp::Add,
                left: Box::new(Expr::Ident("b".to_string())),
                right: Box::new(Expr::Ident("c".to_string())),
            }),
        };
        let captures = analyzer.analyze(&[], &body);
        assert_eq!(captures.len(), 3);
        assert!(captures.contains("a"));
        assert!(captures.contains("b"));
        assert!(captures.contains("c"));
    }
}
