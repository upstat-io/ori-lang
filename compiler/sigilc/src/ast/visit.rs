// Expression visitor for Sigil AST
//
// Provides a trait for visiting AST expressions and producing results.
// Similar to ir/visit.rs but for the untyped AST (before type checking).
//
// Useful for:
// - Type checking dispatch
// - Evaluation
// - AST-level code generation
//
// Implementors override only the methods for nodes they care about.
// Adding a new Expr variant only requires adding a default here.

use super::expr::Expr;
use super::matching::{MatchArm, MatchExpr, Pattern as MatchPattern};
use super::operators::{BinaryOp, UnaryOp};
use super::patterns::PatternExpr;

/// Trait for visiting AST expressions and producing results.
/// Override only the methods you need - defaults handle recursion.
///
/// The trait is parameterized by:
/// - `R`: The result type (e.g., Value for eval, TypeExpr for type checking)
///
/// Implementors must provide:
/// - `default_result()`: The base case result
/// - `combine_results()`: How to merge results from subexpressions
pub trait ExprVisitor {
    /// The result type produced by visiting
    type Result;

    /// The default result when no transformation occurs
    fn default_result(&self) -> Self::Result;

    /// Combine two results (for binary ops, etc.)
    fn combine_results(&self, a: Self::Result, b: Self::Result) -> Self::Result;

    /// Combine multiple results
    fn combine_many(&self, results: Vec<Self::Result>) -> Self::Result {
        results
            .into_iter()
            .fold(self.default_result(), |acc, r| self.combine_results(acc, r))
    }

    /// Main entry point - usually don't override this
    fn visit_expr(&mut self, expr: &Expr) -> Self::Result {
        match expr {
            Expr::Int(n) => self.visit_int(*n),
            Expr::Float(f) => self.visit_float(*f),
            Expr::String(s) => self.visit_string(s),
            Expr::Bool(b) => self.visit_bool(*b),
            Expr::Nil => self.visit_nil(),

            Expr::Ident(name) => self.visit_ident(name),
            Expr::Config(name) => self.visit_config(name),
            Expr::LengthPlaceholder => self.visit_length_placeholder(),

            Expr::List(elems) => self.visit_list(elems),
            Expr::MapLiteral(entries) => self.visit_map_literal(entries),
            Expr::Tuple(elems) => self.visit_tuple(elems),
            Expr::Struct { name, fields } => self.visit_struct(name, fields),

            Expr::Field(obj, field) => self.visit_field(obj, field),
            Expr::Index(obj, idx) => self.visit_index(obj, idx),

            Expr::Call { func, args } => self.visit_call(func, args),
            Expr::MethodCall {
                receiver,
                method,
                args,
            } => self.visit_method_call(receiver, method, args),

            Expr::Binary { op, left, right } => self.visit_binary(*op, left, right),
            Expr::Unary { op, operand } => self.visit_unary(*op, operand),

            Expr::Lambda { params, body } => self.visit_lambda(params, body),

            Expr::Match(m) => self.visit_match(m),
            Expr::If {
                condition,
                then_branch,
                else_branch,
            } => self.visit_if(condition, then_branch, else_branch.as_deref()),
            Expr::For {
                binding,
                iterator,
                body,
            } => self.visit_for(binding, iterator, body),
            Expr::Block(exprs) => self.visit_block(exprs),

            Expr::Range { start, end } => self.visit_range(start, end),

            Expr::Pattern(p) => self.visit_pattern(p),

            Expr::Ok(inner) => self.visit_ok(inner),
            Expr::Err(inner) => self.visit_err(inner),
            Expr::Some(inner) => self.visit_some(inner),
            Expr::None_ => self.visit_none(),
            Expr::Coalesce { value, default } => self.visit_coalesce(value, default),
            Expr::Unwrap(inner) => self.visit_unwrap(inner),

            Expr::Let { name, mutable, value } => self.visit_let(name, *mutable, value),
            Expr::Reassign { target, value } => self.visit_reassign(target, value),
        }
    }

    // === Literals (leaf nodes) ===

    fn visit_int(&mut self, _n: i64) -> Self::Result {
        self.default_result()
    }

    fn visit_float(&mut self, _f: f64) -> Self::Result {
        self.default_result()
    }

    fn visit_string(&mut self, _s: &str) -> Self::Result {
        self.default_result()
    }

    fn visit_bool(&mut self, _b: bool) -> Self::Result {
        self.default_result()
    }

    fn visit_nil(&mut self) -> Self::Result {
        self.default_result()
    }

    // === Variables (leaf nodes) ===

    fn visit_ident(&mut self, _name: &str) -> Self::Result {
        self.default_result()
    }

    fn visit_config(&mut self, _name: &str) -> Self::Result {
        self.default_result()
    }

    fn visit_length_placeholder(&mut self) -> Self::Result {
        self.default_result()
    }

    // === Collections ===

    fn visit_list(&mut self, elems: &[Expr]) -> Self::Result {
        let results: Vec<_> = elems.iter().map(|e| self.visit_expr(e)).collect();
        self.combine_many(results)
    }

    fn visit_map_literal(&mut self, entries: &[(Expr, Expr)]) -> Self::Result {
        let results: Vec<_> = entries
            .iter()
            .flat_map(|(k, v)| vec![self.visit_expr(k), self.visit_expr(v)])
            .collect();
        self.combine_many(results)
    }

    fn visit_tuple(&mut self, elems: &[Expr]) -> Self::Result {
        let results: Vec<_> = elems.iter().map(|e| self.visit_expr(e)).collect();
        self.combine_many(results)
    }

    fn visit_struct(&mut self, _name: &str, fields: &[(String, Expr)]) -> Self::Result {
        let results: Vec<_> = fields.iter().map(|(_, e)| self.visit_expr(e)).collect();
        self.combine_many(results)
    }

    // === Access ===

    fn visit_field(&mut self, obj: &Expr, _field: &str) -> Self::Result {
        self.visit_expr(obj)
    }

    fn visit_index(&mut self, obj: &Expr, idx: &Expr) -> Self::Result {
        let o = self.visit_expr(obj);
        let i = self.visit_expr(idx);
        self.combine_results(o, i)
    }

    // === Calls ===

    fn visit_call(&mut self, func: &Expr, args: &[Expr]) -> Self::Result {
        let f = self.visit_expr(func);
        let arg_results: Vec<_> = args.iter().map(|a| self.visit_expr(a)).collect();
        self.combine_results(f, self.combine_many(arg_results))
    }

    fn visit_method_call(
        &mut self,
        receiver: &Expr,
        _method: &str,
        args: &[Expr],
    ) -> Self::Result {
        let r = self.visit_expr(receiver);
        let arg_results: Vec<_> = args.iter().map(|a| self.visit_expr(a)).collect();
        self.combine_results(r, self.combine_many(arg_results))
    }

    // === Operations ===

    fn visit_binary(&mut self, _op: BinaryOp, left: &Expr, right: &Expr) -> Self::Result {
        let l = self.visit_expr(left);
        let r = self.visit_expr(right);
        self.combine_results(l, r)
    }

    fn visit_unary(&mut self, _op: UnaryOp, operand: &Expr) -> Self::Result {
        self.visit_expr(operand)
    }

    // === Lambda ===

    fn visit_lambda(&mut self, _params: &[String], body: &Expr) -> Self::Result {
        self.visit_expr(body)
    }

    // === Control flow ===

    fn visit_match(&mut self, m: &MatchExpr) -> Self::Result {
        let s = self.visit_expr(&m.scrutinee);
        let arm_results: Vec<_> = m
            .arms
            .iter()
            .map(|arm| {
                let p = self.visit_match_pattern(&arm.pattern);
                let b = self.visit_expr(&arm.body);
                self.combine_results(p, b)
            })
            .collect();
        self.combine_results(s, self.combine_many(arm_results))
    }

    fn visit_match_pattern(&mut self, pattern: &MatchPattern) -> Self::Result {
        match pattern {
            MatchPattern::Condition(expr) => self.visit_expr(expr),
            MatchPattern::Literal(expr) => self.visit_expr(expr),
            _ => self.default_result(),
        }
    }

    fn visit_match_arm(&mut self, arm: &MatchArm) -> Self::Result {
        let p = self.visit_match_pattern(&arm.pattern);
        let b = self.visit_expr(&arm.body);
        self.combine_results(p, b)
    }

    fn visit_if(
        &mut self,
        condition: &Expr,
        then_branch: &Expr,
        else_branch: Option<&Expr>,
    ) -> Self::Result {
        let c = self.visit_expr(condition);
        let t = self.visit_expr(then_branch);
        let e = else_branch.map(|e| self.visit_expr(e)).unwrap_or_else(|| self.default_result());
        self.combine_results(c, self.combine_results(t, e))
    }

    fn visit_for(&mut self, _binding: &str, iterator: &Expr, body: &Expr) -> Self::Result {
        let i = self.visit_expr(iterator);
        let b = self.visit_expr(body);
        self.combine_results(i, b)
    }

    fn visit_let(&mut self, _name: &str, _mutable: bool, value: &Expr) -> Self::Result {
        self.visit_expr(value)
    }

    fn visit_reassign(&mut self, _target: &str, value: &Expr) -> Self::Result {
        self.visit_expr(value)
    }

    fn visit_block(&mut self, exprs: &[Expr]) -> Self::Result {
        let results: Vec<_> = exprs.iter().map(|e| self.visit_expr(e)).collect();
        self.combine_many(results)
    }

    // === Range ===

    fn visit_range(&mut self, start: &Expr, end: &Expr) -> Self::Result {
        let s = self.visit_expr(start);
        let e = self.visit_expr(end);
        self.combine_results(s, e)
    }

    // === Patterns ===

    fn visit_pattern(&mut self, pattern: &PatternExpr) -> Self::Result {
        self.visit_pattern_expr(pattern)
    }

    fn visit_pattern_expr(&mut self, pattern: &PatternExpr) -> Self::Result {
        match pattern {
            PatternExpr::Fold {
                collection,
                init,
                op,
            } => {
                let c = self.visit_expr(collection);
                let i = self.visit_expr(init);
                let o = self.visit_expr(op);
                self.combine_results(c, self.combine_results(i, o))
            }
            PatternExpr::Map {
                collection,
                transform,
            } => {
                let c = self.visit_expr(collection);
                let t = self.visit_expr(transform);
                self.combine_results(c, t)
            }
            PatternExpr::Filter {
                collection,
                predicate,
            } => {
                let c = self.visit_expr(collection);
                let p = self.visit_expr(predicate);
                self.combine_results(c, p)
            }
            PatternExpr::Collect { range, transform } => {
                let r = self.visit_expr(range);
                let t = self.visit_expr(transform);
                self.combine_results(r, t)
            }
            PatternExpr::Recurse {
                condition,
                base_value,
                step,
                ..
            } => {
                let c = self.visit_expr(condition);
                let b = self.visit_expr(base_value);
                let s = self.visit_expr(step);
                self.combine_results(c, self.combine_results(b, s))
            }
            PatternExpr::Iterate {
                over, into, with, ..
            } => {
                let o = self.visit_expr(over);
                let i = self.visit_expr(into);
                let w = self.visit_expr(with);
                self.combine_results(o, self.combine_results(i, w))
            }
            PatternExpr::Transform { input, steps } => {
                let i = self.visit_expr(input);
                let step_results: Vec<_> = steps.iter().map(|s| self.visit_expr(s)).collect();
                self.combine_results(i, self.combine_many(step_results))
            }
            PatternExpr::Count {
                collection,
                predicate,
            } => {
                let c = self.visit_expr(collection);
                let p = self.visit_expr(predicate);
                self.combine_results(c, p)
            }
            PatternExpr::Parallel { branches, timeout, .. } => {
                let mut results: Vec<_> = branches.iter().map(|(_, e)| self.visit_expr(e)).collect();
                if let Some(t) = timeout {
                    results.push(self.visit_expr(t));
                }
                self.combine_many(results)
            }
        }
    }

    // === Result/Option ===

    fn visit_ok(&mut self, inner: &Expr) -> Self::Result {
        self.visit_expr(inner)
    }

    fn visit_err(&mut self, inner: &Expr) -> Self::Result {
        self.visit_expr(inner)
    }

    fn visit_some(&mut self, inner: &Expr) -> Self::Result {
        self.visit_expr(inner)
    }

    fn visit_none(&mut self) -> Self::Result {
        self.default_result()
    }

    fn visit_coalesce(&mut self, value: &Expr, default: &Expr) -> Self::Result {
        let v = self.visit_expr(value);
        let d = self.visit_expr(default);
        self.combine_results(v, d)
    }

    fn visit_unwrap(&mut self, inner: &Expr) -> Self::Result {
        self.visit_expr(inner)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Example visitor that counts expression nodes
    struct ExprCounter;

    impl ExprVisitor for ExprCounter {
        type Result = usize;

        fn default_result(&self) -> usize {
            1
        }

        fn combine_results(&self, a: usize, b: usize) -> usize {
            a + b
        }
    }

    #[test]
    fn test_count_literal() {
        let mut counter = ExprCounter;
        let expr = Expr::Int(42);
        assert_eq!(counter.visit_expr(&expr), 1);
    }

    #[test]
    fn test_count_list() {
        let mut counter = ExprCounter;
        let expr = Expr::List(vec![Expr::Int(1), Expr::Int(2)]);
        // With combine as addition and default as 1:
        // visit_list combines: 1 + 1 = 2
        // combine_many starts with default_result(1) and adds each: 1 + 1 + 1 = 3
        assert_eq!(counter.visit_expr(&expr), 3);
    }

    #[test]
    fn test_count_binary() {
        let mut counter = ExprCounter;
        let expr = Expr::Binary {
            op: BinaryOp::Add,
            left: Box::new(Expr::Int(1)),
            right: Box::new(Expr::Int(2)),
        };
        // left(1) + right(1) = 2
        assert_eq!(counter.visit_expr(&expr), 2);
    }

    /// Example visitor that collects all identifiers
    struct IdentCollector {
        idents: Vec<String>,
    }

    impl ExprVisitor for IdentCollector {
        type Result = ();

        fn default_result(&self) {}

        fn combine_results(&self, _a: (), _b: ()) {}

        fn visit_ident(&mut self, name: &str) -> () {
            self.idents.push(name.to_string());
        }
    }

    #[test]
    fn test_collect_idents() {
        let mut collector = IdentCollector { idents: vec![] };
        let expr = Expr::Binary {
            op: BinaryOp::Add,
            left: Box::new(Expr::Ident("x".to_string())),
            right: Box::new(Expr::Ident("y".to_string())),
        };
        collector.visit_expr(&expr);
        assert_eq!(collector.idents, vec!["x", "y"]);
    }
}
