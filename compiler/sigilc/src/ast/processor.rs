// Unified expression processing infrastructure for Sigil AST
//
// This provides a trait that can be implemented by different compiler phases
// (type checking, evaluation, lowering, code generation) to process expressions
// in a consistent way.
//
// The ExprProcessor trait provides a context-aware interface for expression
// processing across compiler phases.

use super::expr::Expr;
use super::matching::MatchExpr;
use super::operators::{BinaryOp, UnaryOp};
use super::patterns::PatternExpr;

/// Trait for expression processors that handle expressions with internal context.
///
/// This is the primary trait for implementing expression processing across
/// compiler phases. Each phase implements this trait with its own internal
/// context, output, and error types.
///
/// Unlike ExprHandler, this trait is designed for processors that maintain
/// internal state (context) and may mutate it while processing.
///
/// # Type Parameters (Associated Types)
/// - `Output`: The output type produced by processing an expression (e.g., TypeExpr, Value, TExpr)
/// - `Error`: The error type for processing failures
///
/// # Example
/// ```ignore
/// struct TypeChecker<'a> {
///     ctx: &'a mut TypeContext,
/// }
///
/// impl ExprProcessor for TypeChecker<'_> {
///     type Output = TypeExpr;
///     type Error = String;
///
///     fn process_int(&mut self, _n: i64) -> Result<Self::Output, Self::Error> {
///         Ok(TypeExpr::Named("int".to_string()))
///     }
///     // ... implement other methods
/// }
/// ```
pub trait ExprProcessor: Sized {
    /// The output type produced by processing an expression
    type Output;
    /// The error type for processing failures
    type Error;

    /// Process an expression by dispatching to the appropriate handler method
    fn process(&mut self, expr: &Expr) -> Result<Self::Output, Self::Error> {
        dispatch_to_processor(self, expr)
    }

    // === Literal Handlers ===

    /// Handle an integer literal
    fn process_int(&mut self, n: i64) -> Result<Self::Output, Self::Error>;

    /// Handle a float literal
    fn process_float(&mut self, f: f64) -> Result<Self::Output, Self::Error>;

    /// Handle a string literal
    fn process_string(&mut self, s: &str) -> Result<Self::Output, Self::Error>;

    /// Handle a boolean literal
    fn process_bool(&mut self, b: bool) -> Result<Self::Output, Self::Error>;

    /// Handle nil
    fn process_nil(&mut self) -> Result<Self::Output, Self::Error>;

    // === Reference Handlers ===

    /// Handle an identifier reference
    fn process_ident(&mut self, name: &str) -> Result<Self::Output, Self::Error>;

    /// Handle a config reference ($name)
    fn process_config(&mut self, name: &str) -> Result<Self::Output, Self::Error>;

    /// Handle the length placeholder (#)
    fn process_length_placeholder(&mut self) -> Result<Self::Output, Self::Error>;

    // === Collection Handlers ===

    /// Handle a list literal
    fn process_list(&mut self, elems: &[Expr]) -> Result<Self::Output, Self::Error>;

    /// Handle a map literal
    fn process_map_literal(&mut self, entries: &[(Expr, Expr)]) -> Result<Self::Output, Self::Error>;

    /// Handle a tuple literal
    fn process_tuple(&mut self, elems: &[Expr]) -> Result<Self::Output, Self::Error>;

    /// Handle a struct literal
    fn process_struct(&mut self, name: &str, fields: &[(String, Expr)]) -> Result<Self::Output, Self::Error>;

    // === Operation Handlers ===

    /// Handle a binary operation
    fn process_binary(&mut self, op: BinaryOp, left: &Expr, right: &Expr) -> Result<Self::Output, Self::Error>;

    /// Handle a unary operation
    fn process_unary(&mut self, op: UnaryOp, operand: &Expr) -> Result<Self::Output, Self::Error>;

    // === Access Handlers ===

    /// Handle field access
    fn process_field(&mut self, obj: &Expr, field: &str) -> Result<Self::Output, Self::Error>;

    /// Handle index access
    fn process_index(&mut self, obj: &Expr, idx: &Expr) -> Result<Self::Output, Self::Error>;

    // === Call Handlers ===

    /// Handle a function call
    fn process_call(&mut self, func: &Expr, args: &[Expr]) -> Result<Self::Output, Self::Error>;

    /// Handle a method call
    fn process_method_call(
        &mut self,
        receiver: &Expr,
        method: &str,
        args: &[Expr],
    ) -> Result<Self::Output, Self::Error>;

    // === Lambda and Closure Handlers ===

    /// Handle a lambda expression
    fn process_lambda(&mut self, params: &[String], body: &Expr) -> Result<Self::Output, Self::Error>;

    // === Control Flow Handlers ===

    /// Handle a match expression
    fn process_match(&mut self, m: &MatchExpr) -> Result<Self::Output, Self::Error>;

    /// Handle an if expression
    fn process_if(
        &mut self,
        condition: &Expr,
        then_branch: &Expr,
        else_branch: Option<&Expr>,
    ) -> Result<Self::Output, Self::Error>;

    /// Handle a for loop
    fn process_for(
        &mut self,
        binding: &str,
        iterator: &Expr,
        body: &Expr,
    ) -> Result<Self::Output, Self::Error>;

    /// Handle a block expression
    fn process_block(&mut self, exprs: &[Expr]) -> Result<Self::Output, Self::Error>;

    // === Binding Handlers ===

    /// Handle a let binding
    fn process_let(&mut self, name: &str, mutable: bool, value: &Expr) -> Result<Self::Output, Self::Error>;

    /// Handle a reassignment
    fn process_reassign(&mut self, target: &str, value: &Expr) -> Result<Self::Output, Self::Error>;

    // === Range Handler ===

    /// Handle a range expression
    fn process_range(&mut self, start: &Expr, end: &Expr) -> Result<Self::Output, Self::Error>;

    // === Pattern Handler ===

    /// Handle a pattern expression (fold, map, filter, etc.)
    fn process_pattern(&mut self, p: &PatternExpr) -> Result<Self::Output, Self::Error>;

    // === Result/Option Handlers ===

    /// Handle Ok(value)
    fn process_ok(&mut self, inner: &Expr) -> Result<Self::Output, Self::Error>;

    /// Handle Err(value)
    fn process_err(&mut self, inner: &Expr) -> Result<Self::Output, Self::Error>;

    /// Handle Some(value)
    fn process_some(&mut self, inner: &Expr) -> Result<Self::Output, Self::Error>;

    /// Handle None
    fn process_none(&mut self) -> Result<Self::Output, Self::Error>;

    /// Handle value ?? default
    fn process_coalesce(&mut self, value: &Expr, default: &Expr) -> Result<Self::Output, Self::Error>;

    /// Handle value! (unwrap)
    fn process_unwrap(&mut self, inner: &Expr) -> Result<Self::Output, Self::Error>;
}

/// Dispatch an expression to the appropriate processor method
pub fn dispatch_to_processor<P: ExprProcessor>(processor: &mut P, expr: &Expr) -> Result<P::Output, P::Error> {
    match expr {
        Expr::Int(n) => processor.process_int(*n),
        Expr::Float(f) => processor.process_float(*f),
        Expr::String(s) => processor.process_string(s),
        Expr::Bool(b) => processor.process_bool(*b),
        Expr::Nil => processor.process_nil(),
        Expr::Ident(name) => processor.process_ident(name),
        Expr::Config(name) => processor.process_config(name),
        Expr::LengthPlaceholder => processor.process_length_placeholder(),
        Expr::List(elems) => processor.process_list(elems),
        Expr::MapLiteral(entries) => processor.process_map_literal(entries),
        Expr::Tuple(elems) => processor.process_tuple(elems),
        Expr::Struct { name, fields } => processor.process_struct(name, fields),
        Expr::Binary { op, left, right } => processor.process_binary(*op, left, right),
        Expr::Unary { op, operand } => processor.process_unary(*op, operand),
        Expr::Field(obj, field) => processor.process_field(obj, field),
        Expr::Index(obj, idx) => processor.process_index(obj, idx),
        Expr::Call { func, args } => processor.process_call(func, args),
        Expr::MethodCall { receiver, method, args } => {
            processor.process_method_call(receiver, method, args)
        }
        Expr::Lambda { params, body } => processor.process_lambda(params, body),
        Expr::Match(m) => processor.process_match(m),
        Expr::If { condition, then_branch, else_branch } => {
            processor.process_if(condition, then_branch, else_branch.as_deref())
        }
        Expr::For { binding, iterator, body } => processor.process_for(binding, iterator, body),
        Expr::Block(exprs) => processor.process_block(exprs),
        Expr::Let { name, mutable, value } => processor.process_let(name, *mutable, value),
        Expr::Reassign { target, value } => processor.process_reassign(target, value),
        Expr::Range { start, end } => processor.process_range(start, end),
        Expr::Pattern(p) => processor.process_pattern(p),
        Expr::Ok(inner) => processor.process_ok(inner),
        Expr::Err(inner) => processor.process_err(inner),
        Expr::Some(inner) => processor.process_some(inner),
        Expr::None_ => processor.process_none(),
        Expr::Coalesce { value, default } => processor.process_coalesce(value, default),
        Expr::Unwrap(inner) => processor.process_unwrap(inner),
    }
}

/// Trait for processors with default implementations that panic.
///
/// This is useful when you only need to handle a subset of expression types.
/// Implement this trait and override only the methods you need.
pub trait DefaultExprProcessor: Sized {
    /// The output type produced by processing an expression
    type Output;
    /// The error type for processing failures
    type Error;
}

/// Blanket implementation of ExprProcessor for types that implement DefaultExprProcessor.
/// All methods panic by default - override the ones you need.
macro_rules! impl_default_processor {
    ($($method:ident($($arg:ident: $type:ty),*)),* $(,)?) => {
        impl<T: DefaultExprProcessor> ExprProcessor for T {
            type Output = T::Output;
            type Error = T::Error;

            $(
                fn $method(&mut self, $($arg: $type),*) -> Result<Self::Output, Self::Error> {
                    $(let _ = $arg;)*
                    unimplemented!(concat!(stringify!($method), " not implemented"))
                }
            )*
        }
    };
}

// Note: We don't use the macro since it's complex and error-prone.
// Instead, each implementing type can use the ExprProcessor trait directly.

#[cfg(test)]
mod tests {
    use super::*;

    /// Simple processor that just returns the expression variant name
    struct VariantNameProcessor;

    impl ExprProcessor for VariantNameProcessor {
        type Output = &'static str;
        type Error = ();

        fn process_int(&mut self, _: i64) -> Result<Self::Output, Self::Error> {
            Ok("Int")
        }

        fn process_float(&mut self, _: f64) -> Result<Self::Output, Self::Error> {
            Ok("Float")
        }

        fn process_string(&mut self, _: &str) -> Result<Self::Output, Self::Error> {
            Ok("String")
        }

        fn process_bool(&mut self, _: bool) -> Result<Self::Output, Self::Error> {
            Ok("Bool")
        }

        fn process_nil(&mut self) -> Result<Self::Output, Self::Error> {
            Ok("Nil")
        }

        fn process_ident(&mut self, _: &str) -> Result<Self::Output, Self::Error> {
            Ok("Ident")
        }

        fn process_config(&mut self, _: &str) -> Result<Self::Output, Self::Error> {
            Ok("Config")
        }

        fn process_length_placeholder(&mut self) -> Result<Self::Output, Self::Error> {
            Ok("LengthPlaceholder")
        }

        fn process_list(&mut self, _: &[Expr]) -> Result<Self::Output, Self::Error> {
            Ok("List")
        }

        fn process_map_literal(&mut self, _: &[(Expr, Expr)]) -> Result<Self::Output, Self::Error> {
            Ok("MapLiteral")
        }

        fn process_tuple(&mut self, _: &[Expr]) -> Result<Self::Output, Self::Error> {
            Ok("Tuple")
        }

        fn process_struct(&mut self, _: &str, _: &[(String, Expr)]) -> Result<Self::Output, Self::Error> {
            Ok("Struct")
        }

        fn process_binary(&mut self, _: BinaryOp, _: &Expr, _: &Expr) -> Result<Self::Output, Self::Error> {
            Ok("Binary")
        }

        fn process_unary(&mut self, _: UnaryOp, _: &Expr) -> Result<Self::Output, Self::Error> {
            Ok("Unary")
        }

        fn process_field(&mut self, _: &Expr, _: &str) -> Result<Self::Output, Self::Error> {
            Ok("Field")
        }

        fn process_index(&mut self, _: &Expr, _: &Expr) -> Result<Self::Output, Self::Error> {
            Ok("Index")
        }

        fn process_call(&mut self, _: &Expr, _: &[Expr]) -> Result<Self::Output, Self::Error> {
            Ok("Call")
        }

        fn process_method_call(&mut self, _: &Expr, _: &str, _: &[Expr]) -> Result<Self::Output, Self::Error> {
            Ok("MethodCall")
        }

        fn process_lambda(&mut self, _: &[String], _: &Expr) -> Result<Self::Output, Self::Error> {
            Ok("Lambda")
        }

        fn process_match(&mut self, _: &MatchExpr) -> Result<Self::Output, Self::Error> {
            Ok("Match")
        }

        fn process_if(&mut self, _: &Expr, _: &Expr, _: Option<&Expr>) -> Result<Self::Output, Self::Error> {
            Ok("If")
        }

        fn process_for(&mut self, _: &str, _: &Expr, _: &Expr) -> Result<Self::Output, Self::Error> {
            Ok("For")
        }

        fn process_block(&mut self, _: &[Expr]) -> Result<Self::Output, Self::Error> {
            Ok("Block")
        }

        fn process_let(&mut self, _: &str, _: bool, _: &Expr) -> Result<Self::Output, Self::Error> {
            Ok("Let")
        }

        fn process_reassign(&mut self, _: &str, _: &Expr) -> Result<Self::Output, Self::Error> {
            Ok("Reassign")
        }

        fn process_range(&mut self, _: &Expr, _: &Expr) -> Result<Self::Output, Self::Error> {
            Ok("Range")
        }

        fn process_pattern(&mut self, _: &PatternExpr) -> Result<Self::Output, Self::Error> {
            Ok("Pattern")
        }

        fn process_ok(&mut self, _: &Expr) -> Result<Self::Output, Self::Error> {
            Ok("Ok")
        }

        fn process_err(&mut self, _: &Expr) -> Result<Self::Output, Self::Error> {
            Ok("Err")
        }

        fn process_some(&mut self, _: &Expr) -> Result<Self::Output, Self::Error> {
            Ok("Some")
        }

        fn process_none(&mut self) -> Result<Self::Output, Self::Error> {
            Ok("None_")
        }

        fn process_coalesce(&mut self, _: &Expr, _: &Expr) -> Result<Self::Output, Self::Error> {
            Ok("Coalesce")
        }

        fn process_unwrap(&mut self, _: &Expr) -> Result<Self::Output, Self::Error> {
            Ok("Unwrap")
        }
    }

    #[test]
    fn test_process_int() {
        let mut processor = VariantNameProcessor;
        let expr = Expr::Int(42);
        assert_eq!(processor.process(&expr).unwrap(), "Int");
    }

    #[test]
    fn test_process_string() {
        let mut processor = VariantNameProcessor;
        let expr = Expr::String("hello".to_string());
        assert_eq!(processor.process(&expr).unwrap(), "String");
    }

    #[test]
    fn test_process_binary() {
        let mut processor = VariantNameProcessor;
        let expr = Expr::Binary {
            op: BinaryOp::Add,
            left: Box::new(Expr::Int(1)),
            right: Box::new(Expr::Int(2)),
        };
        assert_eq!(processor.process(&expr).unwrap(), "Binary");
    }

    #[test]
    fn test_process_list() {
        let mut processor = VariantNameProcessor;
        let expr = Expr::List(vec![Expr::Int(1), Expr::Int(2)]);
        assert_eq!(processor.process(&expr).unwrap(), "List");
    }
}
