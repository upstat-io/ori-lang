// AST-specific traversal helpers
//
// This module provides specialized traversal utilities for the untyped AST,
// including convenience functions and pre-built traversers for common tasks.

use super::ExprTraversal;
use crate::ast::Expr;
use std::collections::HashSet;

/// A simple collector that gathers identifiers from an expression.
/// Example of using ExprTraversal with AUTO_RECURSE = true.
pub struct IdentCollector {
    pub identifiers: HashSet<String>,
}

impl IdentCollector {
    pub fn new() -> Self {
        IdentCollector {
            identifiers: HashSet::new(),
        }
    }

    /// Collect all identifiers from an expression
    pub fn collect(expr: &Expr) -> HashSet<String> {
        let mut collector = Self::new();
        let _ = collector.traverse(expr);
        collector.identifiers
    }
}

impl Default for IdentCollector {
    fn default() -> Self {
        Self::new()
    }
}

impl ExprTraversal for IdentCollector {
    const AUTO_RECURSE: bool = true;

    type Output = ();
    type Error = std::convert::Infallible;

    fn default_result(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }

    fn combine_results(&mut self, _a: (), _b: ()) -> () {}

    fn on_ident(&mut self, name: &str) -> Result<(), Self::Error> {
        self.identifiers.insert(name.to_string());
        Ok(())
    }
}

/// A collector that finds all function calls in an expression.
pub struct CallCollector {
    pub calls: Vec<String>,
}

impl CallCollector {
    pub fn new() -> Self {
        CallCollector { calls: Vec::new() }
    }

    /// Collect all function call names from an expression
    pub fn collect(expr: &Expr) -> Vec<String> {
        let mut collector = Self::new();
        let _ = collector.traverse(expr);
        collector.calls
    }
}

impl Default for CallCollector {
    fn default() -> Self {
        Self::new()
    }
}

impl ExprTraversal for CallCollector {
    const AUTO_RECURSE: bool = true;

    type Output = ();
    type Error = std::convert::Infallible;

    fn default_result(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }

    fn combine_results(&mut self, _a: (), _b: ()) -> () {}

    fn on_call(&mut self, func: &Expr, args: &[Expr]) -> Result<(), Self::Error> {
        // Record the function name if it's an identifier
        if let Expr::Ident(name) = func {
            self.calls.push(name.clone());
        }
        // Continue recursing into func and args
        self.traverse(func)?;
        for arg in args {
            self.traverse(arg)?;
        }
        Ok(())
    }
}

/// Analyze free variables in an expression (variables used but not bound).
/// Useful for lambda capture analysis.
pub struct FreeVarAnalyzer {
    /// Variables that are in scope (parameters, let bindings, etc.)
    bound: HashSet<String>,
    /// Variables used but not bound
    pub free: HashSet<String>,
}

impl FreeVarAnalyzer {
    pub fn new() -> Self {
        FreeVarAnalyzer {
            bound: HashSet::new(),
            free: HashSet::new(),
        }
    }

    pub fn with_bound(bound: HashSet<String>) -> Self {
        FreeVarAnalyzer {
            bound,
            free: HashSet::new(),
        }
    }

    /// Find free variables in an expression
    pub fn analyze(expr: &Expr) -> HashSet<String> {
        let mut analyzer = Self::new();
        let _ = analyzer.traverse(expr);
        analyzer.free
    }

    /// Find free variables given a set of bound names
    pub fn analyze_with_bound(expr: &Expr, bound: HashSet<String>) -> HashSet<String> {
        let mut analyzer = Self::with_bound(bound);
        let _ = analyzer.traverse(expr);
        analyzer.free
    }
}

impl Default for FreeVarAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl ExprTraversal for FreeVarAnalyzer {
    const AUTO_RECURSE: bool = true;

    type Output = ();
    type Error = std::convert::Infallible;

    fn default_result(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }

    fn combine_results(&mut self, _a: (), _b: ()) -> () {}

    fn on_ident(&mut self, name: &str) -> Result<(), Self::Error> {
        if !self.bound.contains(name) {
            self.free.insert(name.to_string());
        }
        Ok(())
    }

    fn on_let(&mut self, name: &str, _mutable: bool, value: &Expr) -> Result<(), Self::Error> {
        // First analyze the value (before adding name to bound)
        self.traverse(value)?;
        // Then add to bound set for subsequent expressions
        self.bound.insert(name.to_string());
        Ok(())
    }

    fn on_lambda(&mut self, params: &[String], body: &Expr) -> Result<(), Self::Error> {
        // Create new bound set with parameters
        let old_bound = self.bound.clone();
        for param in params {
            self.bound.insert(param.clone());
        }
        // Analyze body
        self.traverse(body)?;
        // Restore bound set
        self.bound = old_bound;
        Ok(())
    }

    fn on_for(&mut self, binding: &str, iterator: &Expr, body: &Expr) -> Result<(), Self::Error> {
        // Analyze iterator (before adding binding)
        self.traverse(iterator)?;
        // Add binding for body
        let had_binding = self.bound.contains(binding);
        self.bound.insert(binding.to_string());
        self.traverse(body)?;
        // Restore if wasn't bound before
        if !had_binding {
            self.bound.remove(binding);
        }
        Ok(())
    }
}

/// Count the depth of nesting in an expression.
pub struct DepthCounter {
    pub max_depth: usize,
    current_depth: usize,
}

impl DepthCounter {
    pub fn new() -> Self {
        DepthCounter {
            max_depth: 0,
            current_depth: 0,
        }
    }

    /// Find the maximum nesting depth
    pub fn measure(expr: &Expr) -> usize {
        let mut counter = Self::new();
        let _ = counter.traverse(expr);
        counter.max_depth
    }
}

impl Default for DepthCounter {
    fn default() -> Self {
        Self::new()
    }
}

impl ExprTraversal for DepthCounter {
    const AUTO_RECURSE: bool = false; // Manual recursion for depth tracking

    type Output = ();
    type Error = std::convert::Infallible;

    fn default_result(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }

    fn combine_results(&mut self, _a: (), _b: ()) -> () {}

    // Override traverse to track depth
    fn traverse(&mut self, expr: &Expr) -> Result<(), Self::Error> {
        self.current_depth += 1;
        if self.current_depth > self.max_depth {
            self.max_depth = self.current_depth;
        }

        // Process this expression's children
        match expr {
            Expr::Int(_) | Expr::Float(_) | Expr::String(_) | Expr::Bool(_) | Expr::Nil => {}
            Expr::Ident(_) | Expr::Config(_) | Expr::LengthPlaceholder | Expr::None_ => {}

            Expr::List(elems) => {
                for e in elems {
                    self.traverse(e)?;
                }
            }
            Expr::Tuple(elems) => {
                for e in elems {
                    self.traverse(e)?;
                }
            }
            Expr::MapLiteral(entries) => {
                for (k, v) in entries {
                    self.traverse(k)?;
                    self.traverse(v)?;
                }
            }
            Expr::Struct { fields, .. } => {
                for (_, e) in fields {
                    self.traverse(e)?;
                }
            }
            Expr::Field(obj, _) => self.traverse(obj)?,
            Expr::Index(obj, idx) => {
                self.traverse(obj)?;
                self.traverse(idx)?;
            }
            Expr::Call { func, args } => {
                self.traverse(func)?;
                for a in args {
                    self.traverse(a)?;
                }
            }
            Expr::MethodCall { receiver, args, .. } => {
                self.traverse(receiver)?;
                for a in args {
                    self.traverse(a)?;
                }
            }
            Expr::Binary { left, right, .. } => {
                self.traverse(left)?;
                self.traverse(right)?;
            }
            Expr::Unary { operand, .. } => self.traverse(operand)?,
            Expr::Lambda { body, .. } => self.traverse(body)?,
            Expr::Match(m) => {
                self.traverse(&m.scrutinee)?;
                for arm in &m.arms {
                    self.traverse(&arm.body)?;
                }
            }
            Expr::If { condition, then_branch, else_branch } => {
                self.traverse(condition)?;
                self.traverse(then_branch)?;
                if let Some(eb) = else_branch {
                    self.traverse(eb)?;
                }
            }
            Expr::For { iterator, body, .. } => {
                self.traverse(iterator)?;
                self.traverse(body)?;
            }
            Expr::Block(exprs) => {
                for e in exprs {
                    self.traverse(e)?;
                }
            }
            Expr::Range { start, end } => {
                self.traverse(start)?;
                self.traverse(end)?;
            }
            Expr::Pattern(_) => {} // Don't recurse into patterns for depth
            Expr::Ok(inner) | Expr::Err(inner) | Expr::Some(inner) | Expr::Unwrap(inner) => {
                self.traverse(inner)?;
            }
            Expr::Coalesce { value, default } => {
                self.traverse(value)?;
                self.traverse(default)?;
            }
            Expr::Let { value, .. } => self.traverse(value)?,
            Expr::Reassign { value, .. } => self.traverse(value)?,
            Expr::With { implementation, body, .. } => {
                self.traverse(implementation)?;
                self.traverse(body)?;
            }
            Expr::Await(inner) => self.traverse(inner)?,
        }

        self.current_depth -= 1;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ident_collector() {
        let expr = Expr::Binary {
            op: crate::ast::BinaryOp::Add,
            left: Box::new(Expr::Ident("x".to_string())),
            right: Box::new(Expr::Ident("y".to_string())),
        };
        let idents = IdentCollector::collect(&expr);
        assert!(idents.contains("x"));
        assert!(idents.contains("y"));
        assert_eq!(idents.len(), 2);
    }

    #[test]
    fn test_free_var_analyzer() {
        // Lambda that captures 'y': x -> x + y
        let expr = Expr::Lambda {
            params: vec!["x".to_string()],
            body: Box::new(Expr::Binary {
                op: crate::ast::BinaryOp::Add,
                left: Box::new(Expr::Ident("x".to_string())),
                right: Box::new(Expr::Ident("y".to_string())),
            }),
        };
        let free = FreeVarAnalyzer::analyze(&expr);
        assert!(!free.contains("x")); // x is bound by lambda
        assert!(free.contains("y")); // y is free
    }

    #[test]
    fn test_depth_counter() {
        // Nested expression: (1 + 2) + 3
        let expr = Expr::Binary {
            op: crate::ast::BinaryOp::Add,
            left: Box::new(Expr::Binary {
                op: crate::ast::BinaryOp::Add,
                left: Box::new(Expr::Int(1)),
                right: Box::new(Expr::Int(2)),
            }),
            right: Box::new(Expr::Int(3)),
        };
        let depth = DepthCounter::measure(&expr);
        assert_eq!(depth, 3); // outer binary -> inner binary -> int literal
    }
}
