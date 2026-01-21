// Unified expression traversal infrastructure for Sigil
//
// This module provides a single traversal trait that can be configured for
// different use cases:
// - Type checking (produces types, may error)
// - Evaluation (produces values, may error)
// - Lowering (produces TIR expressions)
// - Code generation (produces strings)
//
// The key insight is that all expression processing follows the same pattern:
// 1. Dispatch based on expression variant
// 2. Process subexpressions (recursively or manually)
// 3. Combine results into final output
//
// The difference between use cases is:
// - What context is needed (TypeContext, Environment, etc.)
// - Whether recursion is automatic or manual
// - What the output and error types are

pub mod ast;
pub mod tir;

use crate::ast::{BinaryOp, Expr, MatchExpr, PatternExpr, UnaryOp};

/// Unified trait for traversing AST expressions.
///
/// This trait provides a flexible interface for expression processing that
/// supports both automatic and manual recursion strategies.
///
/// # Configuration via Constants
/// - `AUTO_RECURSE`: When true, default implementations automatically recurse
///   into subexpressions. When false, implementors must handle recursion manually.
///
/// # Associated Types
/// - `Context`: Additional context passed to handlers (e.g., TypeContext)
/// - `Output`: The result of processing an expression
/// - `Error`: Error type for failures
///
/// # Usage Patterns
///
/// ## Automatic Recursion (Analysis)
/// For passes that collect information without transforming:
/// ```ignore
/// impl ExprTraversal for FreeVarAnalyzer {
///     const AUTO_RECURSE: bool = true;
///     // Override only the methods you care about
///     fn on_ident(&mut self, name: &str) -> Result<Self::Output, Self::Error> {
///         self.free_vars.insert(name.to_string());
///         self.default_result()
///     }
/// }
/// ```
///
/// ## Manual Recursion (Transformation)
/// For passes that transform expressions:
/// ```ignore
/// impl ExprTraversal for TypeChecker<'_> {
///     const AUTO_RECURSE: bool = false;
///     // Must handle all variants, control recursion explicitly
/// }
/// ```
pub trait ExprTraversal: Sized {
    /// Whether to automatically recurse into subexpressions.
    /// - true: Default implementations call traverse() on children
    /// - false: Implementor must handle all recursion manually
    const AUTO_RECURSE: bool;

    /// Output type produced by traversal
    type Output;

    /// Error type for traversal failures
    type Error;

    /// Create a default/empty result (used for combining)
    fn default_result(&mut self) -> Result<Self::Output, Self::Error>;

    /// Combine two results (for automatic recursion)
    fn combine_results(&mut self, a: Self::Output, b: Self::Output) -> Self::Output;

    /// Combine multiple results
    fn combine_many(&mut self, results: Vec<Self::Output>) -> Self::Output {
        let default = match self.default_result() {
            Ok(d) => d,
            Err(_) => return results.into_iter().next().unwrap_or_else(|| {
                // Fallback - should not happen in normal use
                panic!("combine_many called with empty results and failing default_result")
            }),
        };
        results.into_iter().fold(default, |acc, r| self.combine_results(acc, r))
    }

    // =========================================================================
    // Main Entry Point
    // =========================================================================

    /// Traverse an expression. Usually don't override this.
    fn traverse(&mut self, expr: &Expr) -> Result<Self::Output, Self::Error> {
        match expr {
            // Literals
            Expr::Int(n) => self.on_int(*n),
            Expr::Float(f) => self.on_float(*f),
            Expr::String(s) => self.on_string(s),
            Expr::Bool(b) => self.on_bool(*b),
            Expr::Nil => self.on_nil(),

            // References
            Expr::Ident(name) => self.on_ident(name),
            Expr::Config(name) => self.on_config(name),
            Expr::LengthPlaceholder => self.on_length_placeholder(),

            // Collections
            Expr::List(elems) => self.on_list(elems),
            Expr::MapLiteral(entries) => self.on_map_literal(entries),
            Expr::Tuple(elems) => self.on_tuple(elems),
            Expr::Struct { name, fields } => self.on_struct(name, fields),

            // Access
            Expr::Field(obj, field) => self.on_field(obj, field),
            Expr::Index(obj, idx) => self.on_index(obj, idx),

            // Calls
            Expr::Call { func, args } => self.on_call(func, args),
            Expr::MethodCall { receiver, method, args } => {
                self.on_method_call(receiver, method, args)
            }

            // Operations
            Expr::Binary { op, left, right } => self.on_binary(*op, left, right),
            Expr::Unary { op, operand } => self.on_unary(*op, operand),

            // Lambda
            Expr::Lambda { params, body } => self.on_lambda(params, body),

            // Control Flow
            Expr::Match(m) => self.on_match(m),
            Expr::If { condition, then_branch, else_branch } => {
                self.on_if(condition, then_branch, else_branch.as_deref())
            }
            Expr::For { binding, iterator, body } => self.on_for(binding, iterator, body),
            Expr::Block(exprs) => self.on_block(exprs),
            Expr::Range { start, end } => self.on_range(start, end),

            // Patterns
            Expr::Pattern(p) => self.on_pattern(p),

            // Result/Option
            Expr::Ok(inner) => self.on_ok(inner),
            Expr::Err(inner) => self.on_err(inner),
            Expr::Some(inner) => self.on_some(inner),
            Expr::None_ => self.on_none(),
            Expr::Coalesce { value, default } => self.on_coalesce(value, default),
            Expr::Unwrap(inner) => self.on_unwrap(inner),

            // Bindings (in block context)
            Expr::Let { name, mutable, value } => self.on_let(name, *mutable, value),
            Expr::Reassign { target, value } => self.on_reassign(target, value),
        }
    }

    // =========================================================================
    // Literal Handlers
    // =========================================================================

    fn on_int(&mut self, _n: i64) -> Result<Self::Output, Self::Error> {
        self.default_result()
    }

    fn on_float(&mut self, _f: f64) -> Result<Self::Output, Self::Error> {
        self.default_result()
    }

    fn on_string(&mut self, _s: &str) -> Result<Self::Output, Self::Error> {
        self.default_result()
    }

    fn on_bool(&mut self, _b: bool) -> Result<Self::Output, Self::Error> {
        self.default_result()
    }

    fn on_nil(&mut self) -> Result<Self::Output, Self::Error> {
        self.default_result()
    }

    // =========================================================================
    // Reference Handlers
    // =========================================================================

    fn on_ident(&mut self, _name: &str) -> Result<Self::Output, Self::Error> {
        self.default_result()
    }

    fn on_config(&mut self, _name: &str) -> Result<Self::Output, Self::Error> {
        self.default_result()
    }

    fn on_length_placeholder(&mut self) -> Result<Self::Output, Self::Error> {
        self.default_result()
    }

    // =========================================================================
    // Collection Handlers
    // =========================================================================

    fn on_list(&mut self, elems: &[Expr]) -> Result<Self::Output, Self::Error> {
        if Self::AUTO_RECURSE {
            let results: Result<Vec<_>, _> = elems.iter().map(|e| self.traverse(e)).collect();
            Ok(self.combine_many(results?))
        } else {
            self.default_result()
        }
    }

    fn on_map_literal(&mut self, entries: &[(Expr, Expr)]) -> Result<Self::Output, Self::Error> {
        if Self::AUTO_RECURSE {
            let mut results = Vec::new();
            for (k, v) in entries {
                results.push(self.traverse(k)?);
                results.push(self.traverse(v)?);
            }
            Ok(self.combine_many(results))
        } else {
            self.default_result()
        }
    }

    fn on_tuple(&mut self, elems: &[Expr]) -> Result<Self::Output, Self::Error> {
        if Self::AUTO_RECURSE {
            let results: Result<Vec<_>, _> = elems.iter().map(|e| self.traverse(e)).collect();
            Ok(self.combine_many(results?))
        } else {
            self.default_result()
        }
    }

    fn on_struct(&mut self, _name: &str, fields: &[(String, Expr)]) -> Result<Self::Output, Self::Error> {
        if Self::AUTO_RECURSE {
            let results: Result<Vec<_>, _> = fields.iter().map(|(_, e)| self.traverse(e)).collect();
            Ok(self.combine_many(results?))
        } else {
            self.default_result()
        }
    }

    // =========================================================================
    // Access Handlers
    // =========================================================================

    fn on_field(&mut self, obj: &Expr, _field: &str) -> Result<Self::Output, Self::Error> {
        if Self::AUTO_RECURSE {
            self.traverse(obj)
        } else {
            self.default_result()
        }
    }

    fn on_index(&mut self, obj: &Expr, idx: &Expr) -> Result<Self::Output, Self::Error> {
        if Self::AUTO_RECURSE {
            let obj_result = self.traverse(obj)?;
            let idx_result = self.traverse(idx)?;
            Ok(self.combine_results(obj_result, idx_result))
        } else {
            self.default_result()
        }
    }

    // =========================================================================
    // Call Handlers
    // =========================================================================

    fn on_call(&mut self, func: &Expr, args: &[Expr]) -> Result<Self::Output, Self::Error> {
        if Self::AUTO_RECURSE {
            let func_result = self.traverse(func)?;
            let arg_results: Result<Vec<_>, _> = args.iter().map(|a| self.traverse(a)).collect();
            let combined_args = self.combine_many(arg_results?);
            Ok(self.combine_results(func_result, combined_args))
        } else {
            self.default_result()
        }
    }

    fn on_method_call(
        &mut self,
        receiver: &Expr,
        _method: &str,
        args: &[Expr],
    ) -> Result<Self::Output, Self::Error> {
        if Self::AUTO_RECURSE {
            let recv_result = self.traverse(receiver)?;
            let arg_results: Result<Vec<_>, _> = args.iter().map(|a| self.traverse(a)).collect();
            let combined_args = self.combine_many(arg_results?);
            Ok(self.combine_results(recv_result, combined_args))
        } else {
            self.default_result()
        }
    }

    // =========================================================================
    // Operation Handlers
    // =========================================================================

    fn on_binary(
        &mut self,
        _op: BinaryOp,
        left: &Expr,
        right: &Expr,
    ) -> Result<Self::Output, Self::Error> {
        if Self::AUTO_RECURSE {
            let left_result = self.traverse(left)?;
            let right_result = self.traverse(right)?;
            Ok(self.combine_results(left_result, right_result))
        } else {
            self.default_result()
        }
    }

    fn on_unary(&mut self, _op: UnaryOp, operand: &Expr) -> Result<Self::Output, Self::Error> {
        if Self::AUTO_RECURSE {
            self.traverse(operand)
        } else {
            self.default_result()
        }
    }

    // =========================================================================
    // Lambda Handler
    // =========================================================================

    fn on_lambda(&mut self, _params: &[String], body: &Expr) -> Result<Self::Output, Self::Error> {
        if Self::AUTO_RECURSE {
            self.traverse(body)
        } else {
            self.default_result()
        }
    }

    // =========================================================================
    // Control Flow Handlers
    // =========================================================================

    fn on_match(&mut self, m: &MatchExpr) -> Result<Self::Output, Self::Error> {
        if Self::AUTO_RECURSE {
            let mut results = vec![self.traverse(&m.scrutinee)?];
            for arm in &m.arms {
                results.push(self.traverse(&arm.body)?);
            }
            Ok(self.combine_many(results))
        } else {
            self.default_result()
        }
    }

    fn on_if(
        &mut self,
        condition: &Expr,
        then_branch: &Expr,
        else_branch: Option<&Expr>,
    ) -> Result<Self::Output, Self::Error> {
        if Self::AUTO_RECURSE {
            let cond = self.traverse(condition)?;
            let then_result = self.traverse(then_branch)?;
            let combined = self.combine_results(cond, then_result);
            if let Some(else_expr) = else_branch {
                let else_result = self.traverse(else_expr)?;
                Ok(self.combine_results(combined, else_result))
            } else {
                Ok(combined)
            }
        } else {
            self.default_result()
        }
    }

    fn on_for(
        &mut self,
        _binding: &str,
        iterator: &Expr,
        body: &Expr,
    ) -> Result<Self::Output, Self::Error> {
        if Self::AUTO_RECURSE {
            let iter_result = self.traverse(iterator)?;
            let body_result = self.traverse(body)?;
            Ok(self.combine_results(iter_result, body_result))
        } else {
            self.default_result()
        }
    }

    fn on_block(&mut self, exprs: &[Expr]) -> Result<Self::Output, Self::Error> {
        if Self::AUTO_RECURSE {
            let results: Result<Vec<_>, _> = exprs.iter().map(|e| self.traverse(e)).collect();
            Ok(self.combine_many(results?))
        } else {
            self.default_result()
        }
    }

    fn on_range(&mut self, start: &Expr, end: &Expr) -> Result<Self::Output, Self::Error> {
        if Self::AUTO_RECURSE {
            let start_result = self.traverse(start)?;
            let end_result = self.traverse(end)?;
            Ok(self.combine_results(start_result, end_result))
        } else {
            self.default_result()
        }
    }

    // =========================================================================
    // Pattern Handler
    // =========================================================================

    fn on_pattern(&mut self, _p: &PatternExpr) -> Result<Self::Output, Self::Error> {
        // Patterns have complex structure - default to not recursing
        self.default_result()
    }

    // =========================================================================
    // Result/Option Handlers
    // =========================================================================

    fn on_ok(&mut self, inner: &Expr) -> Result<Self::Output, Self::Error> {
        if Self::AUTO_RECURSE {
            self.traverse(inner)
        } else {
            self.default_result()
        }
    }

    fn on_err(&mut self, inner: &Expr) -> Result<Self::Output, Self::Error> {
        if Self::AUTO_RECURSE {
            self.traverse(inner)
        } else {
            self.default_result()
        }
    }

    fn on_some(&mut self, inner: &Expr) -> Result<Self::Output, Self::Error> {
        if Self::AUTO_RECURSE {
            self.traverse(inner)
        } else {
            self.default_result()
        }
    }

    fn on_none(&mut self) -> Result<Self::Output, Self::Error> {
        self.default_result()
    }

    fn on_coalesce(&mut self, value: &Expr, default: &Expr) -> Result<Self::Output, Self::Error> {
        if Self::AUTO_RECURSE {
            let val = self.traverse(value)?;
            let def = self.traverse(default)?;
            Ok(self.combine_results(val, def))
        } else {
            self.default_result()
        }
    }

    fn on_unwrap(&mut self, inner: &Expr) -> Result<Self::Output, Self::Error> {
        if Self::AUTO_RECURSE {
            self.traverse(inner)
        } else {
            self.default_result()
        }
    }

    // =========================================================================
    // Binding Handlers
    // =========================================================================

    fn on_let(&mut self, _name: &str, _mutable: bool, value: &Expr) -> Result<Self::Output, Self::Error> {
        if Self::AUTO_RECURSE {
            self.traverse(value)
        } else {
            self.default_result()
        }
    }

    fn on_reassign(&mut self, _target: &str, value: &Expr) -> Result<Self::Output, Self::Error> {
        if Self::AUTO_RECURSE {
            self.traverse(value)
        } else {
            self.default_result()
        }
    }
}

/// Helper to traverse a spanned expression
pub fn traverse_spanned<T: ExprTraversal>(
    traverser: &mut T,
    spanned: &crate::ast::SpannedExpr,
) -> Result<T::Output, T::Error> {
    traverser.traverse(&spanned.expr)
}
