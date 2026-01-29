//! Tests for the type checker.

use crate::checker::types::TypedModule;
use crate::checker::{TypeChecker, TypeCheckerBuilder};
use ori_ir::{SharedInterner, TypeId};
use ori_types::{SharedTypeInterner, Type};

/// Result of `check_source` including the type interner for verifying compound types.
struct CheckResult {
    parsed: ori_parse::ParseOutput,
    typed: TypedModule,
    type_interner: SharedTypeInterner,
}

fn check_source(source: &str) -> (ori_parse::ParseOutput, TypedModule) {
    let result = check_source_with_interner(source);
    (result.parsed, result.typed)
}

fn check_source_with_interner(source: &str) -> CheckResult {
    let interner = SharedInterner::default();
    let type_interner = SharedTypeInterner::new();
    let tokens = ori_lexer::lex(source, &interner);
    let parsed = ori_parse::parse(&tokens, &interner);
    // Use builder to pass the type interner
    let checker = TypeCheckerBuilder::new(&parsed.arena, &interner)
        .with_type_interner(type_interner.clone())
        .build();
    let typed = checker.check_module(&parsed.module);
    CheckResult {
        parsed,
        typed,
        type_interner,
    }
}

#[test]
fn test_literal_types() {
    let (parsed, typed) = check_source("@main () -> int = 42");

    assert!(!typed.has_errors());
    assert_eq!(typed.function_types.len(), 1);

    let func = &parsed.module.functions[0];
    let body_type = typed.expr_types[func.body.index()];
    assert_eq!(body_type, TypeId::INT);
}

#[test]
fn test_binary_arithmetic() {
    let (parsed, typed) = check_source("@add () -> int = 1 + 2");

    assert!(!typed.has_errors());

    let func = &parsed.module.functions[0];
    let body_type = typed.expr_types[func.body.index()];
    assert_eq!(body_type, TypeId::INT);
}

#[test]
fn test_comparison() {
    let (parsed, typed) = check_source("@cmp () -> bool = 1 < 2");

    assert!(!typed.has_errors());

    let func = &parsed.module.functions[0];
    let body_type = typed.expr_types[func.body.index()];
    assert_eq!(body_type, TypeId::BOOL);
}

#[test]
fn test_if_expression() {
    let (parsed, typed) = check_source("@test () -> int = if true then 1 else 2");

    assert!(!typed.has_errors());

    let func = &parsed.module.functions[0];
    let body_type = typed.expr_types[func.body.index()];
    assert_eq!(body_type, TypeId::INT);
}

#[test]
fn test_list_type() {
    use ori_types::TypeData;

    let result = check_source_with_interner("@test () = [1, 2, 3]");

    assert!(!result.typed.has_errors());

    let func = &result.parsed.module.functions[0];
    let body_type = result.typed.expr_types[func.body.index()];
    // Verify it's a List(Int) by looking up in the shared interner
    let type_data = result.type_interner.lookup(body_type);
    match type_data {
        TypeData::List(elem_id) => {
            assert_eq!(elem_id, TypeId::INT, "List element should be int");
        }
        _ => panic!("Expected List type, got {type_data:?}"),
    }
}

#[test]
fn test_type_mismatch_error() {
    let (_, typed) = check_source("@test () -> int = if 42 then 1 else 2");

    assert!(typed.has_errors());
    assert!(
        typed.errors[0].message.contains("type mismatch")
            || typed.errors[0].message.contains("expected")
    );
}

#[test]
fn test_let_type_annotation_mismatch() {
    // Simpler test - just the let binding
    let source = r#"@main () -> void = let x: int = "hello""#;
    let (parsed, typed) = check_source(source);

    eprintln!("Parse errors: {:?}", parsed.errors);
    eprintln!("Type errors: {:?}", typed.errors);
    assert!(
        typed.has_errors(),
        "Should have type error for let x: int = \"hello\""
    );
}

#[test]
fn test_let_type_annotation_mismatch_in_run() {
    // Test inside run pattern - with void return
    let source = r#"@main () -> void = run(let x: int = "hello", ())"#;
    let (parsed, typed) = check_source(source);

    eprintln!("Parse errors: {:?}", parsed.errors);
    eprintln!("Type errors: {:?}", typed.errors);
    // Should catch the int/str mismatch, not just return type
    let has_int_str_error = typed
        .errors
        .iter()
        .any(|e| e.message.contains("int") && e.message.contains("str"));
    assert!(
        has_int_str_error,
        "Should have type error for let x: int = \"hello\" inside run"
    );
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
    assert_eq!(func_type.params[0], TypeId::INT);
    assert_eq!(func_type.params[1], TypeId::INT);
    assert_eq!(func_type.return_type, TypeId::INT);
}

#[test]
fn test_function_call_type_inference() {
    let (_, typed) = check_source("@double (x: int) -> int = x * 2");

    assert!(!typed.has_errors());
    assert_eq!(typed.function_types.len(), 1);

    let func_type = &typed.function_types[0];
    assert_eq!(func_type.return_type, TypeId::INT);
}

#[test]
fn test_lambda_with_typed_param() {
    let (_, typed) = check_source("@test () = (x: int) -> x + 1");

    assert!(!typed.has_errors());
}

#[test]
fn test_tuple_type() {
    use ori_types::TypeData;

    let result = check_source_with_interner("@test () = (1, true, \"hello\")");

    assert!(!result.typed.has_errors());

    let func = &result.parsed.module.functions[0];
    let body_type = result.typed.expr_types[func.body.index()];
    // Verify it's a Tuple(Int, Bool, Str) by looking up in the shared interner
    let type_data = result.type_interner.lookup(body_type);
    match type_data {
        TypeData::Tuple(elem_ids) => {
            assert_eq!(elem_ids.len(), 3, "Tuple should have 3 elements");
            assert_eq!(elem_ids[0], TypeId::INT, "First element should be int");
            assert_eq!(elem_ids[1], TypeId::BOOL, "Second element should be bool");
            assert_eq!(elem_ids[2], TypeId::STR, "Third element should be str");
        }
        _ => panic!("Expected Tuple type, got {type_data:?}"),
    }
}

#[test]
fn test_nested_if_type() {
    let (_, typed) = check_source(
        r"
            @test (x: int) -> int =
                if x > 0 then
                    if x > 10 then 100 else 10
                else
                    0
        ",
    );

    assert!(!typed.has_errors());
}

#[test]
fn test_run_pattern_type() {
    let (_, typed) = check_source(
        r"
            @test () -> int = run(
                let x: int = 1,
                let y: int = 2,
                x + y
            )
        ",
    );

    assert!(!typed.has_errors());
}

#[test]
fn test_closure_self_capture_direct() {
    let (_, typed) = check_source(
        r"
            @test () -> int = run(
                let f = () -> f,
                0
            )
        ",
    );

    assert!(typed.has_errors());
    assert!(typed
        .errors
        .iter()
        .any(|e| e.message.contains("closure cannot capture itself")
            && e.code == ori_diagnostic::ErrorCode::E2007));
}

#[test]
fn test_closure_self_capture_call() {
    let (_, typed) = check_source(
        r"
            @test () -> int = run(
                let f = (x: int) -> f(x + 1),
                0
            )
        ",
    );

    assert!(typed.has_errors());
    assert!(typed
        .errors
        .iter()
        .any(|e| e.message.contains("closure cannot capture itself")));
}

#[test]
fn test_no_self_capture_uses_outer_binding() {
    let (_, typed) = check_source(
        r"
            @test () -> int = run(
                let f = 42,
                let g = () -> f,
                g()
            )
        ",
    );

    assert!(!typed
        .errors
        .iter()
        .any(|e| e.code == ori_diagnostic::ErrorCode::E2007));
}

#[test]
fn test_no_self_capture_non_lambda() {
    let (_, typed) = check_source(
        r"
            @test () -> int = run(
                let x = 1 + 2,
                x
            )
        ",
    );

    assert!(!typed.has_errors());
}

#[test]
fn test_closure_self_capture_in_run() {
    let (_, typed) = check_source(
        r"
            @test () -> int = run(
                let f = () -> f,
                0
            )
        ",
    );

    assert!(typed.has_errors());
    assert!(typed
        .errors
        .iter()
        .any(|e| e.message.contains("closure cannot capture itself")));
}

#[test]
fn test_closure_self_capture_nested_expression() {
    let (_, typed) = check_source(
        r"
            @test () -> int = run(
                let f = () -> if true then f else f,
                0
            )
        ",
    );

    assert!(typed.has_errors());
    assert!(typed
        .errors
        .iter()
        .any(|e| e.message.contains("closure cannot capture itself")));
}

#[test]
fn test_valid_mutual_recursion_via_outer_scope() {
    let (_, typed) = check_source(
        r"
            @f (x: int) -> int = x
            @test () -> int = run(
                let g = (x: int) -> @f(x),
                g(1)
            )
        ",
    );

    assert!(!typed
        .errors
        .iter()
        .any(|e| e.code == ori_diagnostic::ErrorCode::E2007));
}

#[test]
fn test_type_registry_in_checker() {
    let interner = SharedInterner::default();
    let tokens = ori_lexer::lex("@main () -> int = 42", &interner);
    let parsed = ori_parse::parse(&tokens, &interner);

    let mut checker = TypeChecker::new(&parsed.arena, &interner);

    let point_name = interner.intern("Point");
    let x_name = interner.intern("x");
    let y_name = interner.intern("y");

    let type_id = checker.registries.types.register_struct(
        point_name,
        vec![(x_name, Type::Int), (y_name, Type::Int)],
        ori_ir::Span::new(0, 0),
        vec![],
    );

    assert!(checker.registries.types.contains(point_name));
    let entry = checker.registries.types.get_by_id(type_id).unwrap();
    assert_eq!(entry.name, point_name);
}

#[test]
fn test_type_id_to_type_with_newtype() {
    let interner = SharedInterner::default();
    let tokens = ori_lexer::lex("@main () -> int = 42", &interner);
    let parsed = ori_parse::parse(&tokens, &interner);

    let mut checker = TypeChecker::new(&parsed.arena, &interner);

    let id_name = interner.intern("UserId");
    let type_id = checker.registries.types.register_newtype(
        id_name,
        &Type::Int,
        ori_ir::Span::new(0, 0),
        vec![],
    );

    // Newtypes are nominally distinct - they resolve to Type::Named, not the underlying type
    let resolved = checker.type_id_to_type(type_id);
    assert_eq!(resolved, Type::Named(id_name));
}

#[test]
fn test_type_id_to_type_with_struct() {
    let interner = SharedInterner::default();
    let tokens = ori_lexer::lex("@main () -> int = 42", &interner);
    let parsed = ori_parse::parse(&tokens, &interner);

    let mut checker = TypeChecker::new(&parsed.arena, &interner);

    let point_name = interner.intern("Point");
    let type_id = checker.registries.types.register_struct(
        point_name,
        vec![],
        ori_ir::Span::new(0, 0),
        vec![],
    );

    let resolved = checker.type_id_to_type(type_id);
    assert_eq!(resolved, Type::Named(point_name));
}

#[test]
fn test_module_namespace_registration() {
    use crate::checker::imports::{ImportedFunction, ImportedModuleAlias};

    let interner = SharedInterner::default();
    let tokens = ori_lexer::lex("@main () -> int = 42", &interner);
    let parsed = ori_parse::parse(&tokens, &interner);

    let mut checker = TypeChecker::new(&parsed.arena, &interner);

    // Create a module alias with two functions
    let alias_name = interner.intern("math");
    let add_name = interner.intern("add");
    let subtract_name = interner.intern("subtract");

    let module_alias = ImportedModuleAlias {
        alias: alias_name,
        functions: vec![
            ImportedFunction {
                name: add_name,
                params: vec![Type::Int, Type::Int],
                return_type: Type::Int,
                generics: vec![],
                capabilities: vec![],
            },
            ImportedFunction {
                name: subtract_name,
                params: vec![Type::Int, Type::Int],
                return_type: Type::Int,
                generics: vec![],
                capabilities: vec![],
            },
        ],
    };

    checker.register_module_alias(&module_alias);

    // Verify the module alias is bound in the environment
    let resolved = checker.inference.env.lookup(alias_name);
    assert!(resolved.is_some(), "Module alias should be bound");

    let ty = resolved.unwrap();
    match ty {
        Type::ModuleNamespace { items } => {
            assert_eq!(items.len(), 2, "Namespace should have 2 items");
            assert_eq!(items[0].0, add_name);
            assert_eq!(items[1].0, subtract_name);
        }
        _ => panic!("Expected ModuleNamespace type, got {:?}", ty),
    }
}

#[test]
fn test_module_namespace_field_access_type() {
    use crate::checker::imports::{ImportedFunction, ImportedModuleAlias};

    let interner = SharedInterner::default();
    let tokens = ori_lexer::lex("@main () -> int = 42", &interner);
    let parsed = ori_parse::parse(&tokens, &interner);

    let mut checker = TypeChecker::new(&parsed.arena, &interner);

    // Register a module alias
    let alias_name = interner.intern("math");
    let add_name = interner.intern("add");

    let module_alias = ImportedModuleAlias {
        alias: alias_name,
        functions: vec![ImportedFunction {
            name: add_name,
            params: vec![Type::Int, Type::Int],
            return_type: Type::Int,
            generics: vec![],
            capabilities: vec![],
        }],
    };

    checker.register_module_alias(&module_alias);

    // Verify field access on the namespace returns the function type
    let ns_type = checker.inference.env.lookup(alias_name).unwrap();
    if let Type::ModuleNamespace { items } = ns_type {
        // Look up "add" in the namespace
        let add_type = items.iter().find(|(name, _)| *name == add_name).map(|(_, ty)| ty);
        assert!(add_type.is_some(), "Should find 'add' in namespace");

        let fn_type = add_type.unwrap();
        match fn_type {
            Type::Function { params, ret } => {
                assert_eq!(params.len(), 2);
                assert_eq!(params[0], Type::Int);
                assert_eq!(params[1], Type::Int);
                assert_eq!(**ret, Type::Int);
            }
            _ => panic!("Expected Function type, got {:?}", fn_type),
        }
    } else {
        panic!("Expected ModuleNamespace");
    }
}
