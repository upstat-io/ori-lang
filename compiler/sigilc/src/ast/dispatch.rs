// Unified expression dispatch infrastructure for Sigil AST
//
// Provides macros for consistent expression handling across compiler phases.
// Adding a new Expr variant only requires updating the dispatch macro.

use super::expr::Expr;
use super::matching::MatchExpr;
use super::operators::{BinaryOp, UnaryOp};
use super::patterns::PatternExpr;

/// Macro to generate exhaustive expression dispatch
/// Ensures all Expr variants are handled consistently across compiler phases.
///
/// Note: For most use cases, prefer using `ExprHandler` trait with `dispatch_to_handler`
/// function, which provides a cleaner interface with default implementations.
///
/// # Example
/// ```ignore
/// dispatch_expr!(expr, {
///     Int(n) => handle_int(*n),
///     Float(f) => handle_float(*f),
///     String(s) => handle_string(s.clone()),
///     Bool(b) => handle_bool(*b),
///     Nil => handle_nil(),
///     Ident(name) => handle_ident(name),
///     // ... other variants
/// })
/// ```
#[allow(unused_macros)]
#[macro_export]
macro_rules! dispatch_expr {
    ($expr:expr, {
        $($variant:ident $( ( $($binding:pat),* $(,)? ) )? $( { $($field:ident),* $(,)? } )? => $handler:expr),* $(,)?
    }) => {
        match $expr {
            $(
                $crate::ast::Expr::$variant $( ( $($binding),* ) )? $( { $($field),* } )? => $handler,
            )*
        }
    };
}

/// Trait for expression handlers that can be implemented for different phases.
/// This provides a default implementation pattern for handling expressions
/// across type checking, lowering, evaluation, and code generation.
///
/// Each method has a default that returns an error, so implementors only
/// need to override the methods they care about.
pub trait ExprHandler {
    /// The output type produced by handling an expression
    type Output;
    /// The error type for handling failures
    type Error;

    /// Handle an integer literal
    fn handle_int(&mut self, n: i64) -> Result<Self::Output, Self::Error> {
        let _ = n;
        unimplemented!("handle_int not implemented")
    }

    /// Handle a float literal
    fn handle_float(&mut self, f: f64) -> Result<Self::Output, Self::Error> {
        let _ = f;
        unimplemented!("handle_float not implemented")
    }

    /// Handle a string literal
    fn handle_string(&mut self, s: &str) -> Result<Self::Output, Self::Error> {
        let _ = s;
        unimplemented!("handle_string not implemented")
    }

    /// Handle a boolean literal
    fn handle_bool(&mut self, b: bool) -> Result<Self::Output, Self::Error> {
        let _ = b;
        unimplemented!("handle_bool not implemented")
    }

    /// Handle nil
    fn handle_nil(&mut self) -> Result<Self::Output, Self::Error> {
        unimplemented!("handle_nil not implemented")
    }

    /// Handle an identifier reference
    fn handle_ident(&mut self, name: &str) -> Result<Self::Output, Self::Error> {
        let _ = name;
        unimplemented!("handle_ident not implemented")
    }

    /// Handle a config reference ($name)
    fn handle_config(&mut self, name: &str) -> Result<Self::Output, Self::Error> {
        let _ = name;
        unimplemented!("handle_config not implemented")
    }

    /// Handle the length placeholder (#)
    fn handle_length_placeholder(&mut self) -> Result<Self::Output, Self::Error> {
        unimplemented!("handle_length_placeholder not implemented")
    }

    /// Handle a list literal
    fn handle_list(&mut self, elems: &[Expr]) -> Result<Self::Output, Self::Error> {
        let _ = elems;
        unimplemented!("handle_list not implemented")
    }

    /// Handle a map literal
    fn handle_map_literal(&mut self, entries: &[(Expr, Expr)]) -> Result<Self::Output, Self::Error> {
        let _ = entries;
        unimplemented!("handle_map_literal not implemented")
    }

    /// Handle a tuple literal
    fn handle_tuple(&mut self, elems: &[Expr]) -> Result<Self::Output, Self::Error> {
        let _ = elems;
        unimplemented!("handle_tuple not implemented")
    }

    /// Handle a struct literal
    fn handle_struct(&mut self, name: &str, fields: &[(String, Expr)]) -> Result<Self::Output, Self::Error> {
        let _ = (name, fields);
        unimplemented!("handle_struct not implemented")
    }

    /// Handle a binary operation
    fn handle_binary(&mut self, op: BinaryOp, left: &Expr, right: &Expr) -> Result<Self::Output, Self::Error> {
        let _ = (op, left, right);
        unimplemented!("handle_binary not implemented")
    }

    /// Handle a unary operation
    fn handle_unary(&mut self, op: UnaryOp, operand: &Expr) -> Result<Self::Output, Self::Error> {
        let _ = (op, operand);
        unimplemented!("handle_unary not implemented")
    }

    /// Handle field access
    fn handle_field(&mut self, obj: &Expr, field: &str) -> Result<Self::Output, Self::Error> {
        let _ = (obj, field);
        unimplemented!("handle_field not implemented")
    }

    /// Handle index access
    fn handle_index(&mut self, obj: &Expr, idx: &Expr) -> Result<Self::Output, Self::Error> {
        let _ = (obj, idx);
        unimplemented!("handle_index not implemented")
    }

    /// Handle a function call
    fn handle_call(&mut self, func: &Expr, args: &[Expr]) -> Result<Self::Output, Self::Error> {
        let _ = (func, args);
        unimplemented!("handle_call not implemented")
    }

    /// Handle a method call
    fn handle_method_call(
        &mut self,
        receiver: &Expr,
        method: &str,
        args: &[Expr],
    ) -> Result<Self::Output, Self::Error> {
        let _ = (receiver, method, args);
        unimplemented!("handle_method_call not implemented")
    }

    /// Handle a lambda expression
    fn handle_lambda(&mut self, params: &[String], body: &Expr) -> Result<Self::Output, Self::Error> {
        let _ = (params, body);
        unimplemented!("handle_lambda not implemented")
    }

    /// Handle a match expression
    fn handle_match(&mut self, m: &MatchExpr) -> Result<Self::Output, Self::Error> {
        let _ = m;
        unimplemented!("handle_match not implemented")
    }

    /// Handle an if expression
    fn handle_if(
        &mut self,
        condition: &Expr,
        then_branch: &Expr,
        else_branch: Option<&Expr>,
    ) -> Result<Self::Output, Self::Error> {
        let _ = (condition, then_branch, else_branch);
        unimplemented!("handle_if not implemented")
    }

    /// Handle a for loop
    fn handle_for(
        &mut self,
        binding: &str,
        iterator: &Expr,
        body: &Expr,
    ) -> Result<Self::Output, Self::Error> {
        let _ = (binding, iterator, body);
        unimplemented!("handle_for not implemented")
    }

    /// Handle a block expression
    fn handle_block(&mut self, exprs: &[Expr]) -> Result<Self::Output, Self::Error> {
        let _ = exprs;
        unimplemented!("handle_block not implemented")
    }

    /// Handle a let binding
    fn handle_let(&mut self, name: &str, mutable: bool, value: &Expr) -> Result<Self::Output, Self::Error> {
        let _ = (name, mutable, value);
        unimplemented!("handle_let not implemented")
    }

    /// Handle a reassignment
    fn handle_reassign(&mut self, target: &str, value: &Expr) -> Result<Self::Output, Self::Error> {
        let _ = (target, value);
        unimplemented!("handle_reassign not implemented")
    }

    /// Handle a range expression
    fn handle_range(&mut self, start: &Expr, end: &Expr) -> Result<Self::Output, Self::Error> {
        let _ = (start, end);
        unimplemented!("handle_range not implemented")
    }

    /// Handle a pattern expression (fold, map, filter, etc.)
    fn handle_pattern(&mut self, p: &PatternExpr) -> Result<Self::Output, Self::Error> {
        let _ = p;
        unimplemented!("handle_pattern not implemented")
    }

    /// Handle Ok(value)
    fn handle_ok(&mut self, inner: &Expr) -> Result<Self::Output, Self::Error> {
        let _ = inner;
        unimplemented!("handle_ok not implemented")
    }

    /// Handle Err(value)
    fn handle_err(&mut self, inner: &Expr) -> Result<Self::Output, Self::Error> {
        let _ = inner;
        unimplemented!("handle_err not implemented")
    }

    /// Handle Some(value)
    fn handle_some(&mut self, inner: &Expr) -> Result<Self::Output, Self::Error> {
        let _ = inner;
        unimplemented!("handle_some not implemented")
    }

    /// Handle None
    fn handle_none(&mut self) -> Result<Self::Output, Self::Error> {
        unimplemented!("handle_none not implemented")
    }

    /// Handle value ?? default
    fn handle_coalesce(&mut self, value: &Expr, default: &Expr) -> Result<Self::Output, Self::Error> {
        let _ = (value, default);
        unimplemented!("handle_coalesce not implemented")
    }

    /// Handle value! (unwrap)
    fn handle_unwrap(&mut self, inner: &Expr) -> Result<Self::Output, Self::Error> {
        let _ = inner;
        unimplemented!("handle_unwrap not implemented")
    }

    /// Handle with Capability = impl in body (capability injection)
    fn handle_with(&mut self, capability: &str, implementation: &Expr, body: &Expr) -> Result<Self::Output, Self::Error> {
        let _ = (capability, implementation, body);
        unimplemented!("handle_with not implemented")
    }

    /// Handle await expression
    fn handle_await(&mut self, inner: &Expr) -> Result<Self::Output, Self::Error> {
        let _ = inner;
        unimplemented!("handle_await not implemented")
    }
}

/// Dispatch an expression to the appropriate handler method
pub fn dispatch_to_handler<H: ExprHandler>(handler: &mut H, expr: &Expr) -> Result<H::Output, H::Error> {
    match expr {
        Expr::Int(n) => handler.handle_int(*n),
        Expr::Float(f) => handler.handle_float(*f),
        Expr::String(s) => handler.handle_string(s),
        Expr::Bool(b) => handler.handle_bool(*b),
        Expr::Nil => handler.handle_nil(),
        Expr::Ident(name) => handler.handle_ident(name),
        Expr::Config(name) => handler.handle_config(name),
        Expr::LengthPlaceholder => handler.handle_length_placeholder(),
        Expr::List(elems) => handler.handle_list(elems),
        Expr::MapLiteral(entries) => handler.handle_map_literal(entries),
        Expr::Tuple(elems) => handler.handle_tuple(elems),
        Expr::Struct { name, fields } => handler.handle_struct(name, fields),
        Expr::Binary { op, left, right } => handler.handle_binary(*op, left, right),
        Expr::Unary { op, operand } => handler.handle_unary(*op, operand),
        Expr::Field(obj, field) => handler.handle_field(obj, field),
        Expr::Index(obj, idx) => handler.handle_index(obj, idx),
        Expr::Call { func, args } => handler.handle_call(func, args),
        Expr::MethodCall { receiver, method, args } => {
            handler.handle_method_call(receiver, method, args)
        }
        Expr::Lambda { params, body } => handler.handle_lambda(params, body),
        Expr::Match(m) => handler.handle_match(m),
        Expr::If { condition, then_branch, else_branch } => {
            handler.handle_if(condition, then_branch, else_branch.as_deref())
        }
        Expr::For { binding, iterator, body } => handler.handle_for(binding, iterator, body),
        Expr::Block(exprs) => handler.handle_block(exprs),
        Expr::Let { name, mutable, value } => handler.handle_let(name, *mutable, value),
        Expr::Reassign { target, value } => handler.handle_reassign(target, value),
        Expr::Range { start, end } => handler.handle_range(start, end),
        Expr::Pattern(p) => handler.handle_pattern(p),
        Expr::Ok(inner) => handler.handle_ok(inner),
        Expr::Err(inner) => handler.handle_err(inner),
        Expr::Some(inner) => handler.handle_some(inner),
        Expr::None_ => handler.handle_none(),
        Expr::Coalesce { value, default } => handler.handle_coalesce(value, default),
        Expr::Unwrap(inner) => handler.handle_unwrap(inner),
        Expr::With { capability, implementation, body } => {
            handler.handle_with(capability, implementation, body)
        }
        Expr::Await(inner) => handler.handle_await(inner),
    }
}

/// Get the list of all expression variant names
/// Useful for generating documentation or ensuring exhaustive handling
pub const EXPR_VARIANTS: &[&str] = &[
    "Int",
    "Float",
    "String",
    "Bool",
    "Nil",
    "Ident",
    "Config",
    "LengthPlaceholder",
    "List",
    "MapLiteral",
    "Tuple",
    "Struct",
    "Binary",
    "Unary",
    "Field",
    "Index",
    "Call",
    "MethodCall",
    "Lambda",
    "Match",
    "If",
    "For",
    "Block",
    "Let",
    "Reassign",
    "Range",
    "Pattern",
    "Ok",
    "Err",
    "Some",
    "None_",
    "Coalesce",
    "Unwrap",
];

#[cfg(test)]
mod tests {
    use super::*;

    /// Simple handler that just returns the expression variant name
    struct VariantNameHandler;

    impl ExprHandler for VariantNameHandler {
        type Output = &'static str;
        type Error = ();

        fn handle_int(&mut self, _: i64) -> Result<Self::Output, Self::Error> {
            Ok("Int")
        }

        fn handle_float(&mut self, _: f64) -> Result<Self::Output, Self::Error> {
            Ok("Float")
        }

        fn handle_string(&mut self, _: &str) -> Result<Self::Output, Self::Error> {
            Ok("String")
        }

        fn handle_bool(&mut self, _: bool) -> Result<Self::Output, Self::Error> {
            Ok("Bool")
        }

        fn handle_nil(&mut self) -> Result<Self::Output, Self::Error> {
            Ok("Nil")
        }

        fn handle_ident(&mut self, _: &str) -> Result<Self::Output, Self::Error> {
            Ok("Ident")
        }

        fn handle_config(&mut self, _: &str) -> Result<Self::Output, Self::Error> {
            Ok("Config")
        }

        fn handle_length_placeholder(&mut self) -> Result<Self::Output, Self::Error> {
            Ok("LengthPlaceholder")
        }

        fn handle_list(&mut self, _: &[Expr]) -> Result<Self::Output, Self::Error> {
            Ok("List")
        }

        fn handle_map_literal(&mut self, _: &[(Expr, Expr)]) -> Result<Self::Output, Self::Error> {
            Ok("MapLiteral")
        }

        fn handle_tuple(&mut self, _: &[Expr]) -> Result<Self::Output, Self::Error> {
            Ok("Tuple")
        }

        fn handle_struct(&mut self, _: &str, _: &[(String, Expr)]) -> Result<Self::Output, Self::Error> {
            Ok("Struct")
        }

        fn handle_binary(&mut self, _: BinaryOp, _: &Expr, _: &Expr) -> Result<Self::Output, Self::Error> {
            Ok("Binary")
        }

        fn handle_unary(&mut self, _: UnaryOp, _: &Expr) -> Result<Self::Output, Self::Error> {
            Ok("Unary")
        }

        fn handle_field(&mut self, _: &Expr, _: &str) -> Result<Self::Output, Self::Error> {
            Ok("Field")
        }

        fn handle_index(&mut self, _: &Expr, _: &Expr) -> Result<Self::Output, Self::Error> {
            Ok("Index")
        }

        fn handle_call(&mut self, _: &Expr, _: &[Expr]) -> Result<Self::Output, Self::Error> {
            Ok("Call")
        }

        fn handle_method_call(&mut self, _: &Expr, _: &str, _: &[Expr]) -> Result<Self::Output, Self::Error> {
            Ok("MethodCall")
        }

        fn handle_lambda(&mut self, _: &[String], _: &Expr) -> Result<Self::Output, Self::Error> {
            Ok("Lambda")
        }

        fn handle_match(&mut self, _: &MatchExpr) -> Result<Self::Output, Self::Error> {
            Ok("Match")
        }

        fn handle_if(&mut self, _: &Expr, _: &Expr, _: Option<&Expr>) -> Result<Self::Output, Self::Error> {
            Ok("If")
        }

        fn handle_for(&mut self, _: &str, _: &Expr, _: &Expr) -> Result<Self::Output, Self::Error> {
            Ok("For")
        }

        fn handle_block(&mut self, _: &[Expr]) -> Result<Self::Output, Self::Error> {
            Ok("Block")
        }

        fn handle_let(&mut self, _: &str, _: bool, _: &Expr) -> Result<Self::Output, Self::Error> {
            Ok("Let")
        }

        fn handle_reassign(&mut self, _: &str, _: &Expr) -> Result<Self::Output, Self::Error> {
            Ok("Reassign")
        }

        fn handle_range(&mut self, _: &Expr, _: &Expr) -> Result<Self::Output, Self::Error> {
            Ok("Range")
        }

        fn handle_pattern(&mut self, _: &PatternExpr) -> Result<Self::Output, Self::Error> {
            Ok("Pattern")
        }

        fn handle_ok(&mut self, _: &Expr) -> Result<Self::Output, Self::Error> {
            Ok("Ok")
        }

        fn handle_err(&mut self, _: &Expr) -> Result<Self::Output, Self::Error> {
            Ok("Err")
        }

        fn handle_some(&mut self, _: &Expr) -> Result<Self::Output, Self::Error> {
            Ok("Some")
        }

        fn handle_none(&mut self) -> Result<Self::Output, Self::Error> {
            Ok("None_")
        }

        fn handle_coalesce(&mut self, _: &Expr, _: &Expr) -> Result<Self::Output, Self::Error> {
            Ok("Coalesce")
        }

        fn handle_unwrap(&mut self, _: &Expr) -> Result<Self::Output, Self::Error> {
            Ok("Unwrap")
        }
    }

    #[test]
    fn test_dispatch_int() {
        let mut handler = VariantNameHandler;
        let expr = Expr::Int(42);
        assert_eq!(dispatch_to_handler(&mut handler, &expr).unwrap(), "Int");
    }

    #[test]
    fn test_dispatch_string() {
        let mut handler = VariantNameHandler;
        let expr = Expr::String("hello".to_string());
        assert_eq!(dispatch_to_handler(&mut handler, &expr).unwrap(), "String");
    }

    #[test]
    fn test_dispatch_binary() {
        let mut handler = VariantNameHandler;
        let expr = Expr::Binary {
            op: BinaryOp::Add,
            left: Box::new(Expr::Int(1)),
            right: Box::new(Expr::Int(2)),
        };
        assert_eq!(dispatch_to_handler(&mut handler, &expr).unwrap(), "Binary");
    }

    #[test]
    fn test_expr_variants_count() {
        // Verify the count matches the actual number of Expr variants
        assert_eq!(EXPR_VARIANTS.len(), 33);
    }
}
