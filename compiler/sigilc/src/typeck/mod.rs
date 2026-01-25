//! Type checker for Sigil.
//!
//! Implements Hindley-Milner type inference with extensions for
//! Sigil's pattern system.
//!
//! # Module Structure
//!
//! - `checker/`: TypeChecker struct and main entry point
//!   - `mod.rs`: TypeChecker struct, check_module
//!   - `types.rs`: TypedModule, FunctionType, GenericBound
//!   - `signatures.rs`: Function signature inference
//!   - `pattern_binding.rs`: Pattern to type binding
//!   - `cycle_detection.rs`: Closure self-capture detection
//!   - `trait_registration.rs`: Trait and impl registration
//!   - `bound_checking.rs`: Trait bound verification
//! - `operators.rs`: Binary operator type checking
//! - `type_registry/`: User-defined type registration
//!   - `mod.rs`: TypeRegistry
//!   - `trait_registry.rs`: TraitRegistry
//! - `infer/`: Expression type inference
//!   - `mod.rs`: Main infer_expr dispatcher
//!   - `expr.rs`: Literals, identifiers, operators
//!   - `call.rs`: Function and method calls
//!   - `control.rs`: Control flow (if, match, loops)
//!   - `pattern.rs`: Pattern expressions (run, try, map, etc.)

mod checker;
mod infer;
pub mod operators;
pub mod type_registry;

// Re-export main types
pub use checker::{
    TypeChecker, TypedModule, FunctionType, GenericBound, TypeCheckError,
    type_check, type_check_with_context,
};
pub use type_registry::{
    TypeRegistry, TypeEntry, TypeKind, VariantDef,
    TraitRegistry, TraitEntry, TraitMethodDef, TraitAssocTypeDef,
    ImplEntry, ImplMethodDef, MethodLookup, CoherenceError,
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer;
    use crate::parser;
    use crate::ir::SharedInterner;
    use crate::types::Type;

    fn check_source(source: &str) -> (crate::parser::ParseResult, TypedModule) {
        let interner = SharedInterner::default();
        let tokens = lexer::lex(source, &interner);
        let parsed = parser::parse(&tokens, &interner);
        let typed = type_check(&parsed, &interner);
        (parsed, typed)
    }

    #[test]
    fn test_literal_types() {
        let (parsed, typed) = check_source("@main () -> int = 42");

        assert!(!typed.has_errors());
        assert_eq!(typed.function_types.len(), 1);

        let func = &parsed.module.functions[0];
        let body_type = &typed.expr_types[func.body.index()];
        assert_eq!(*body_type, Type::Int);
    }

    #[test]
    fn test_binary_arithmetic() {
        let (parsed, typed) = check_source("@add () -> int = 1 + 2");

        assert!(!typed.has_errors());

        let func = &parsed.module.functions[0];
        let body_type = &typed.expr_types[func.body.index()];
        assert_eq!(*body_type, Type::Int);
    }

    #[test]
    fn test_comparison() {
        let (parsed, typed) = check_source("@cmp () -> bool = 1 < 2");

        assert!(!typed.has_errors());

        let func = &parsed.module.functions[0];
        let body_type = &typed.expr_types[func.body.index()];
        assert_eq!(*body_type, Type::Bool);
    }

    #[test]
    fn test_if_expression() {
        let (parsed, typed) = check_source("@test () -> int = if true then 1 else 2");

        assert!(!typed.has_errors());

        let func = &parsed.module.functions[0];
        let body_type = &typed.expr_types[func.body.index()];
        assert_eq!(*body_type, Type::Int);
    }

    #[test]
    fn test_list_type() {
        let (parsed, typed) = check_source("@test () = [1, 2, 3]");

        assert!(!typed.has_errors());

        let func = &parsed.module.functions[0];
        let body_type = &typed.expr_types[func.body.index()];
        assert_eq!(*body_type, Type::List(Box::new(Type::Int)));
    }

    #[test]
    fn test_type_mismatch_error() {
        let (_, typed) = check_source("@test () -> int = if 42 then 1 else 2");

        assert!(typed.has_errors());
        assert!(typed.errors[0].message.contains("type mismatch") ||
                typed.errors[0].message.contains("expected"));
    }

    #[test]
    fn test_typed_module_salsa_traits() {
        use std::collections::HashSet;

        let (_, typed1) = check_source("@main () -> int = 42");
        let (_, typed2) = check_source("@main () -> int = 42");
        let (_, typed3) = check_source("@main () -> bool = true");

        assert_eq!(typed1, typed2);
        assert_ne!(typed1, typed3);

        let mut set = HashSet::new();
        set.insert(typed1.clone());
        set.insert(typed2);
        set.insert(typed3);
        assert_eq!(set.len(), 2);
    }

    #[test]
    fn test_function_with_typed_params() {
        let (_, typed) = check_source("@add (a: int, b: int) -> int = a + b");

        assert!(!typed.has_errors());
        assert_eq!(typed.function_types.len(), 1);

        let func_type = &typed.function_types[0];
        assert_eq!(func_type.params.len(), 2);
        assert_eq!(func_type.params[0], Type::Int);
        assert_eq!(func_type.params[1], Type::Int);
        assert_eq!(func_type.return_type, Type::Int);
    }

    #[test]
    fn test_function_call_type_inference() {
        let (_, typed) = check_source("@double (x: int) -> int = x * 2");

        assert!(!typed.has_errors());
        assert_eq!(typed.function_types.len(), 1);

        let func_type = &typed.function_types[0];
        assert_eq!(func_type.return_type, Type::Int);
    }

    #[test]
    fn test_lambda_with_typed_param() {
        let (_, typed) = check_source("@test () = (x: int) -> x + 1");

        assert!(!typed.has_errors());
    }

    #[test]
    fn test_tuple_type() {
        let (parsed, typed) = check_source("@test () = (1, true, \"hello\")");

        assert!(!typed.has_errors());

        let func = &parsed.module.functions[0];
        let body_type = &typed.expr_types[func.body.index()];
        assert_eq!(
            *body_type,
            Type::Tuple(vec![Type::Int, Type::Bool, Type::Str])
        );
    }

    #[test]
    fn test_nested_if_type() {
        let (_, typed) = check_source(r#"
            @test (x: int) -> int =
                if x > 0 then
                    if x > 10 then 100 else 10
                else
                    0
        "#);

        assert!(!typed.has_errors());
    }

    #[test]
    fn test_run_pattern_type() {
        let (_, typed) = check_source(r#"
            @test () -> int = run(
                let x: int = 1,
                let y: int = 2,
                x + y
            )
        "#);

        assert!(!typed.has_errors());
    }

    // =========================================================================
    // Closure Self-Capture Detection Tests
    // =========================================================================

    #[test]
    fn test_closure_self_capture_direct() {
        let (_, typed) = check_source(r#"
            @test () -> int = run(
                let f = () -> f,
                0
            )
        "#);

        assert!(typed.has_errors());
        assert!(typed.errors.iter().any(|e|
            e.message.contains("closure cannot capture itself") &&
            e.code == crate::diagnostic::ErrorCode::E2007
        ));
    }

    #[test]
    fn test_closure_self_capture_call() {
        let (_, typed) = check_source(r#"
            @test () -> int = run(
                let f = (x: int) -> f(x + 1),
                0
            )
        "#);

        assert!(typed.has_errors());
        assert!(typed.errors.iter().any(|e|
            e.message.contains("closure cannot capture itself")
        ));
    }

    #[test]
    fn test_no_self_capture_uses_outer_binding() {
        let (_, typed) = check_source(r#"
            @test () -> int = run(
                let f = 42,
                let g = () -> f,
                g()
            )
        "#);

        assert!(!typed.errors.iter().any(|e|
            e.code == crate::diagnostic::ErrorCode::E2007
        ));
    }

    #[test]
    fn test_no_self_capture_non_lambda() {
        let (_, typed) = check_source(r#"
            @test () -> int = run(
                let x = 1 + 2,
                x
            )
        "#);

        assert!(!typed.has_errors());
    }

    #[test]
    fn test_closure_self_capture_in_run() {
        let (_, typed) = check_source(r#"
            @test () -> int = run(
                let f = () -> f,
                0
            )
        "#);

        assert!(typed.has_errors());
        assert!(typed.errors.iter().any(|e|
            e.message.contains("closure cannot capture itself")
        ));
    }

    #[test]
    fn test_closure_self_capture_nested_expression() {
        let (_, typed) = check_source(r#"
            @test () -> int = run(
                let f = () -> if true then f else f,
                0
            )
        "#);

        assert!(typed.has_errors());
        assert!(typed.errors.iter().any(|e|
            e.message.contains("closure cannot capture itself")
        ));
    }

    #[test]
    fn test_valid_mutual_recursion_via_outer_scope() {
        let (_, typed) = check_source(r#"
            @f (x: int) -> int = x
            @test () -> int = run(
                let g = (x: int) -> @f(x),
                g(1)
            )
        "#);

        assert!(!typed.errors.iter().any(|e|
            e.code == crate::diagnostic::ErrorCode::E2007
        ));
    }

    // =========================================================================
    // TypeRegistry Integration Tests
    // =========================================================================

    #[test]
    fn test_type_registry_in_checker() {
        let interner = SharedInterner::default();
        let tokens = lexer::lex("@main () -> int = 42", &interner);
        let parsed = parser::parse(&tokens, &interner);

        let mut checker = TypeChecker::new(&parsed.arena, &interner);

        let point_name = interner.intern("Point");
        let x_name = interner.intern("x");
        let y_name = interner.intern("y");

        let type_id = checker.type_registry.register_struct(
            point_name,
            vec![(x_name, Type::Int), (y_name, Type::Int)],
            crate::ir::Span::new(0, 0),
            vec![],
        );

        assert!(checker.type_registry.contains(point_name));
        let entry = checker.type_registry.get_by_id(type_id).unwrap();
        assert_eq!(entry.name, point_name);
    }

    #[test]
    fn test_type_id_to_type_with_registry() {
        let interner = SharedInterner::default();
        let tokens = lexer::lex("@main () -> int = 42", &interner);
        let parsed = parser::parse(&tokens, &interner);

        let mut checker = TypeChecker::new(&parsed.arena, &interner);

        let id_name = interner.intern("UserId");
        let type_id = checker.type_registry.register_alias(
            id_name,
            Type::Int,
            crate::ir::Span::new(0, 0),
            vec![],
        );

        let resolved = checker.type_id_to_type(type_id);
        assert_eq!(resolved, Type::Int);
    }

    #[test]
    fn test_type_id_to_type_with_struct() {
        let interner = SharedInterner::default();
        let tokens = lexer::lex("@main () -> int = 42", &interner);
        let parsed = parser::parse(&tokens, &interner);

        let mut checker = TypeChecker::new(&parsed.arena, &interner);

        let point_name = interner.intern("Point");
        let type_id = checker.type_registry.register_struct(
            point_name,
            vec![],
            crate::ir::Span::new(0, 0),
            vec![],
        );

        let resolved = checker.type_id_to_type(type_id);
        assert_eq!(resolved, Type::Named(point_name));
    }
}
