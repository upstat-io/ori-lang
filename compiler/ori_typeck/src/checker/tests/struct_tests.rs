//! Tests for struct types, type registry, and module namespaces.

use crate::checker::imports::{ImportedFunction, ImportedModuleAlias};
use crate::checker::TypeChecker;
use ori_ir::SharedInterner;
use ori_types::Type;

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
        _ => panic!("Expected ModuleNamespace type, got {ty:?}"),
    }
}

#[test]
fn test_module_namespace_field_access_type() {
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
    // Use get_namespace_item for O(log n) binary search lookup
    let add_type = ns_type.get_namespace_item(add_name);
    assert!(add_type.is_some(), "Should find 'add' in namespace");
    if matches!(ns_type, Type::ModuleNamespace { .. }) {
        let fn_type = add_type.unwrap();
        match fn_type {
            Type::Function { params, ret } => {
                assert_eq!(params.len(), 2);
                assert_eq!(params[0], Type::Int);
                assert_eq!(params[1], Type::Int);
                assert_eq!(**ret, Type::Int);
            }
            _ => panic!("Expected Function type, got {fn_type:?}"),
        }
    } else {
        panic!("Expected ModuleNamespace");
    }
}
