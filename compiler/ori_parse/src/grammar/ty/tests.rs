use ori_ir::{ExprArena, ParsedType, StringInterner, TypeId};

use crate::Parser;

/// Parse a type from source, returning the type and the arena for lookups.
fn parse_type_with_arena(source: &str) -> (Option<ParsedType>, ExprArena) {
    let interner = StringInterner::new();
    // Wrap in a function to get proper context for type parsing
    let full_source = format!("@test () -> {source} = 0;");
    let tokens = ori_lexer::lex(&full_source, &interner);
    let mut parser = Parser::new(&tokens, &interner);

    // Skip to return type: @test () ->
    parser.cursor.advance(); // @
    parser.cursor.advance(); // test
    parser.cursor.advance(); // (
    parser.cursor.advance(); // )
    parser.cursor.advance(); // ->

    let ty = parser.parse_type();
    let arena = parser.take_arena();
    (ty, arena)
}

#[test]
fn test_parse_primitive_types() {
    let (ty, _) = parse_type_with_arena("int");
    assert_eq!(ty, Some(ParsedType::primitive(TypeId::INT)));

    let (ty, _) = parse_type_with_arena("float");
    assert_eq!(ty, Some(ParsedType::primitive(TypeId::FLOAT)));

    let (ty, _) = parse_type_with_arena("bool");
    assert_eq!(ty, Some(ParsedType::primitive(TypeId::BOOL)));

    let (ty, _) = parse_type_with_arena("str");
    assert_eq!(ty, Some(ParsedType::primitive(TypeId::STR)));

    let (ty, _) = parse_type_with_arena("char");
    assert_eq!(ty, Some(ParsedType::primitive(TypeId::CHAR)));

    let (ty, _) = parse_type_with_arena("byte");
    assert_eq!(ty, Some(ParsedType::primitive(TypeId::BYTE)));

    let (ty, _) = parse_type_with_arena("void");
    assert_eq!(ty, Some(ParsedType::primitive(TypeId::VOID)));

    let (ty, _) = parse_type_with_arena("Never");
    assert_eq!(ty, Some(ParsedType::primitive(TypeId::NEVER)));
}

#[test]
fn test_parse_unit_type() {
    // () is unit (empty tuple)
    let (ty, _) = parse_type_with_arena("()");
    assert!(matches!(ty, Some(ParsedType::Tuple(ref v)) if v.is_empty()));
}

#[test]
fn test_parse_named_type() {
    let (ty, _) = parse_type_with_arena("MyType");
    assert!(matches!(
        ty,
        Some(ParsedType::Named { type_args, .. }) if type_args.is_empty()
    ));
}

#[test]
fn test_parse_generic_type() {
    // Generic types like Option<int>
    let (ty, arena) = parse_type_with_arena("Option<int>");
    match ty {
        Some(ParsedType::Named { type_args, .. }) => {
            assert_eq!(type_args.len(), 1);
            let ids = arena.get_parsed_type_list(type_args);
            assert_eq!(
                *arena.get_parsed_type(ids[0]),
                ParsedType::primitive(TypeId::INT)
            );
        }
        _ => panic!("expected Named with type args"),
    }

    // Result<int, str>
    let (ty, arena) = parse_type_with_arena("Result<int, str>");
    match ty {
        Some(ParsedType::Named { type_args, .. }) => {
            assert_eq!(type_args.len(), 2);
            let ids = arena.get_parsed_type_list(type_args);
            assert_eq!(
                *arena.get_parsed_type(ids[0]),
                ParsedType::primitive(TypeId::INT)
            );
            assert_eq!(
                *arena.get_parsed_type(ids[1]),
                ParsedType::primitive(TypeId::STR)
            );
        }
        _ => panic!("expected Named with 2 type args"),
    }
}

#[test]
fn test_parse_list_type() {
    let (ty, arena) = parse_type_with_arena("[int]");
    match ty {
        Some(ParsedType::List(inner_id)) => {
            assert_eq!(
                *arena.get_parsed_type(inner_id),
                ParsedType::primitive(TypeId::INT)
            );
        }
        _ => panic!("expected List"),
    }

    let (ty, arena) = parse_type_with_arena("[str]");
    match ty {
        Some(ParsedType::List(inner_id)) => {
            assert_eq!(
                *arena.get_parsed_type(inner_id),
                ParsedType::primitive(TypeId::STR)
            );
        }
        _ => panic!("expected List"),
    }
}

#[test]
fn test_parse_tuple_type() {
    let (ty, arena) = parse_type_with_arena("(int, str)");
    match ty {
        Some(ParsedType::Tuple(elems)) => {
            assert_eq!(elems.len(), 2);
            let ids = arena.get_parsed_type_list(elems);
            assert_eq!(
                *arena.get_parsed_type(ids[0]),
                ParsedType::primitive(TypeId::INT)
            );
            assert_eq!(
                *arena.get_parsed_type(ids[1]),
                ParsedType::primitive(TypeId::STR)
            );
        }
        _ => panic!("expected Tuple"),
    }
}

#[test]
fn test_parse_function_type() {
    let (ty, arena) = parse_type_with_arena("() -> int");
    match ty {
        Some(ParsedType::Function { params, ret }) => {
            assert!(params.is_empty());
            assert_eq!(
                *arena.get_parsed_type(ret),
                ParsedType::primitive(TypeId::INT)
            );
        }
        _ => panic!("expected Function"),
    }

    let (ty, arena) = parse_type_with_arena("(int) -> str");
    match ty {
        Some(ParsedType::Function { params, ret }) => {
            assert_eq!(params.len(), 1);
            let param_ids = arena.get_parsed_type_list(params);
            assert_eq!(
                *arena.get_parsed_type(param_ids[0]),
                ParsedType::primitive(TypeId::INT)
            );
            assert_eq!(
                *arena.get_parsed_type(ret),
                ParsedType::primitive(TypeId::STR)
            );
        }
        _ => panic!("expected Function"),
    }

    let (ty, arena) = parse_type_with_arena("(int, str) -> bool");
    match ty {
        Some(ParsedType::Function { params, ret }) => {
            assert_eq!(params.len(), 2);
            assert_eq!(
                *arena.get_parsed_type(ret),
                ParsedType::primitive(TypeId::BOOL)
            );
        }
        _ => panic!("expected Function"),
    }
}

#[test]
fn test_parse_nested_generic_type() {
    // Nested generics like Option<Result<int, str>>
    let (ty, arena) = parse_type_with_arena("Option<Result<int, str>>");
    match ty {
        Some(ParsedType::Named { type_args, .. }) => {
            assert_eq!(type_args.len(), 1);
            let ids = arena.get_parsed_type_list(type_args);
            match arena.get_parsed_type(ids[0]) {
                ParsedType::Named {
                    type_args: inner, ..
                } => {
                    assert_eq!(inner.len(), 2);
                }
                _ => panic!("expected inner Named"),
            }
        }
        _ => panic!("expected Named"),
    }
}

#[test]
fn test_parse_double_nested_generic_type() {
    // Double nested generics: Result<Result<T, E>, E>
    // This was previously broken because >> was lexed as a single Shr token.
    // Now the lexer produces individual > tokens, enabling correct parsing.
    let (ty, arena) = parse_type_with_arena("Result<Result<int, str>, str>");
    match ty {
        Some(ParsedType::Named { type_args, .. }) => {
            assert_eq!(type_args.len(), 2, "Expected 2 type args for outer Result");
            let outer_ids = arena.get_parsed_type_list(type_args);
            // First arg should be Result<int, str>
            match arena.get_parsed_type(outer_ids[0]) {
                ParsedType::Named {
                    type_args: inner, ..
                } => {
                    assert_eq!(inner.len(), 2, "Expected 2 type args for inner Result");
                    let inner_ids = arena.get_parsed_type_list(*inner);
                    assert_eq!(
                        *arena.get_parsed_type(inner_ids[0]),
                        ParsedType::primitive(TypeId::INT)
                    );
                    assert_eq!(
                        *arena.get_parsed_type(inner_ids[1]),
                        ParsedType::primitive(TypeId::STR)
                    );
                }
                _ => panic!("expected inner Named (Result<int, str>)"),
            }
            // Second arg should be str
            assert_eq!(
                *arena.get_parsed_type(outer_ids[1]),
                ParsedType::primitive(TypeId::STR)
            );
        }
        _ => panic!("expected Named"),
    }
}

#[test]
fn test_parse_triple_nested_generic_type() {
    // Triple nested: Option<Result<Result<int, str>, str>>
    let (ty, arena) = parse_type_with_arena("Option<Result<Result<int, str>, str>>");
    match ty {
        Some(ParsedType::Named { type_args, .. }) => {
            assert_eq!(type_args.len(), 1, "Expected 1 type arg for Option");
            let outer_ids = arena.get_parsed_type_list(type_args);
            match arena.get_parsed_type(outer_ids[0]) {
                ParsedType::Named {
                    type_args: inner, ..
                } => {
                    assert_eq!(inner.len(), 2, "Expected 2 type args for outer Result");
                    let inner_ids = arena.get_parsed_type_list(*inner);
                    // First arg should be Result<int, str>
                    match arena.get_parsed_type(inner_ids[0]) {
                        ParsedType::Named {
                            type_args: deepest, ..
                        } => {
                            assert_eq!(deepest.len(), 2, "Expected 2 type args for inner Result");
                        }
                        _ => panic!("expected innermost Named"),
                    }
                }
                _ => panic!("expected inner Named (Result<...>)"),
            }
        }
        _ => panic!("expected Named"),
    }
}

#[test]
fn test_parse_self_type() {
    let (ty, _) = parse_type_with_arena("Self");
    assert_eq!(ty, Some(ParsedType::SelfType));
}

#[test]
fn test_parse_list_of_generic() {
    // [Option<int>]
    let (ty, arena) = parse_type_with_arena("[Option<int>]");
    match ty {
        Some(ParsedType::List(inner_id)) => match arena.get_parsed_type(inner_id) {
            ParsedType::Named { type_args, .. } => {
                assert_eq!(type_args.len(), 1);
            }
            _ => panic!("expected Named inside List"),
        },
        _ => panic!("expected List"),
    }
}

#[test]
fn test_parse_self_associated_type() {
    // Self.Item - associated type access on Self
    let (ty, arena) = parse_type_with_arena("Self.Item");
    match ty {
        Some(ParsedType::AssociatedType { base, assoc_name }) => {
            assert_eq!(*arena.get_parsed_type(base), ParsedType::SelfType);
            // Note: assoc_name is a Name, we just verify it was parsed
            let _ = assoc_name;
        }
        _ => panic!("expected AssociatedType, got {ty:?}"),
    }
}

#[test]
fn test_parse_generic_associated_type() {
    // T.Item - associated type access on a type variable
    let (ty, arena) = parse_type_with_arena("T.Item");
    match ty {
        Some(ParsedType::AssociatedType { base, assoc_name }) => {
            match arena.get_parsed_type(base) {
                ParsedType::Named { type_args, .. } => {
                    assert!(type_args.is_empty());
                }
                _ => panic!("expected Named as base"),
            }
            let _ = assoc_name;
        }
        _ => panic!("expected AssociatedType, got {ty:?}"),
    }
}

#[test]
fn test_parse_option_of_associated_type() {
    // Option<Self.Item> - associated type inside generic
    let (ty, arena) = parse_type_with_arena("Option<Self.Item>");
    match ty {
        Some(ParsedType::Named { type_args, .. }) => {
            assert_eq!(type_args.len(), 1);
            let ids = arena.get_parsed_type_list(type_args);
            match arena.get_parsed_type(ids[0]) {
                ParsedType::AssociatedType { base, .. } => {
                    assert_eq!(*arena.get_parsed_type(*base), ParsedType::SelfType);
                }
                _ => panic!("expected AssociatedType as type arg"),
            }
        }
        _ => panic!("expected Named"),
    }
}

/// Parse a type from source, returning the type, arena, and any deferred errors.
fn parse_type_with_errors(source: &str) -> (Option<ParsedType>, ExprArena, Vec<String>) {
    let interner = StringInterner::new();
    let full_source = format!("@test () -> {source} = 0;");
    let tokens = ori_lexer::lex(&full_source, &interner);
    let mut parser = Parser::new(&tokens, &interner);

    // Skip to return type: @test () ->
    parser.cursor.advance(); // @
    parser.cursor.advance(); // test
    parser.cursor.advance(); // (
    parser.cursor.advance(); // )
    parser.cursor.advance(); // ->

    let ty = parser.parse_type();
    let errors: Vec<String> = parser
        .deferred_errors
        .iter()
        .map(|e| e.message.clone())
        .collect();
    let arena = parser.take_arena();
    (ty, arena, errors)
}

#[test]
fn test_ampersand_type_produces_error() {
    let (ty, _, errors) = parse_type_with_errors("&int");
    // Recovers by parsing the inner type
    assert_eq!(ty, Some(ParsedType::primitive(TypeId::INT)));
    assert_eq!(errors.len(), 1);
    assert!(errors[0].contains("borrowed references"));
}

#[test]
fn test_ampersand_named_type_produces_error() {
    let (ty, _, errors) = parse_type_with_errors("&MyType");
    // Recovers by parsing the inner named type
    assert!(matches!(ty, Some(ParsedType::Named { .. })));
    assert_eq!(errors.len(), 1);
    assert!(errors[0].contains("reserved for a future version"));
}

#[test]
fn test_ampersand_alone_recovers_to_infer() {
    // &= (& followed by =, not a type) should recover to Infer
    let (ty, _, errors) = parse_type_with_errors("&");
    assert_eq!(ty, Some(ParsedType::Infer));
    assert_eq!(errors.len(), 1);
}

#[test]
fn test_parse_fixed_list_integer_literal() {
    // [int, max 42] — regression test: integer literal capacity
    let (ty, arena) = parse_type_with_arena("[int, max 42]");
    match ty {
        Some(ParsedType::FixedList { elem, capacity }) => {
            assert_eq!(
                *arena.get_parsed_type(elem),
                ParsedType::primitive(TypeId::INT)
            );
            let expr = arena.get_expr(capacity);
            assert!(
                matches!(expr.kind, ori_ir::ExprKind::Int(42)),
                "expected Int(42), got {:?}",
                expr.kind
            );
        }
        _ => panic!("expected FixedList, got {ty:?}"),
    }
}

#[test]
fn test_parse_fixed_list_const_param() {
    // [int, max $N] — const generic capacity
    let interner = StringInterner::new();
    let full_source = "@test () -> [int, max $N] = 0;";
    let tokens = ori_lexer::lex(full_source, &interner);
    let mut parser = Parser::new(&tokens, &interner);

    // Skip to return type
    parser.cursor.advance(); // @
    parser.cursor.advance(); // test
    parser.cursor.advance(); // (
    parser.cursor.advance(); // )
    parser.cursor.advance(); // ->

    let ty = parser.parse_type();
    let arena = parser.take_arena();

    match ty {
        Some(ParsedType::FixedList { elem, capacity }) => {
            assert_eq!(
                *arena.get_parsed_type(elem),
                ParsedType::primitive(TypeId::INT)
            );
            let expr = arena.get_expr(capacity);
            assert!(
                matches!(expr.kind, ori_ir::ExprKind::Const(_)),
                "expected Const, got {:?}",
                expr.kind
            );
        }
        _ => panic!("expected FixedList, got {ty:?}"),
    }
}

#[test]
fn test_parse_generic_with_const_expr() {
    // Array<int, $N> — const expression in type argument
    let interner = StringInterner::new();
    let full_source = "@test () -> Array<int, $N> = 0;";
    let tokens = ori_lexer::lex(full_source, &interner);
    let mut parser = Parser::new(&tokens, &interner);

    // Skip to return type
    parser.cursor.advance(); // @
    parser.cursor.advance(); // test
    parser.cursor.advance(); // (
    parser.cursor.advance(); // )
    parser.cursor.advance(); // ->

    let ty = parser.parse_type();
    let arena = parser.take_arena();

    match ty {
        Some(ParsedType::Named { type_args, .. }) => {
            assert_eq!(type_args.len(), 2, "Expected 2 type args");
            let args = arena.get_parsed_type_list(type_args);
            // First arg should be int
            assert_eq!(
                *arena.get_parsed_type(args[0]),
                ParsedType::primitive(TypeId::INT)
            );
            // Second arg should be ConstExpr
            assert!(
                arena.get_parsed_type(args[1]).is_const_expr(),
                "Expected ConstExpr, got {:?}",
                arena.get_parsed_type(args[1])
            );
        }
        _ => panic!("expected Named, got {ty:?}"),
    }
}

#[test]
fn test_parse_generic_with_integer_literal() {
    // Array<int, 10> — integer literal in type argument
    let interner = StringInterner::new();
    let full_source = "@test () -> Array<int, 10> = 0;";
    let tokens = ori_lexer::lex(full_source, &interner);
    let mut parser = Parser::new(&tokens, &interner);

    parser.cursor.advance(); // @
    parser.cursor.advance(); // test
    parser.cursor.advance(); // (
    parser.cursor.advance(); // )
    parser.cursor.advance(); // ->

    let ty = parser.parse_type();
    let arena = parser.take_arena();

    match ty {
        Some(ParsedType::Named { type_args, .. }) => {
            assert_eq!(type_args.len(), 2, "Expected 2 type args");
            let args = arena.get_parsed_type_list(type_args);
            assert_eq!(
                *arena.get_parsed_type(args[0]),
                ParsedType::primitive(TypeId::INT)
            );
            match arena.get_parsed_type(args[1]) {
                ParsedType::ConstExpr(expr_id) => {
                    let expr = arena.get_expr(*expr_id);
                    assert!(
                        matches!(expr.kind, ori_ir::ExprKind::Int(10)),
                        "expected Int(10), got {:?}",
                        expr.kind
                    );
                }
                other => panic!("expected ConstExpr, got {other:?}"),
            }
        }
        _ => panic!("expected Named, got {ty:?}"),
    }
}

#[test]
fn test_parse_trait_bounds_two() {
    // Printable + Hashable — two bounded trait object
    let (ty, arena) = parse_type_with_arena("Printable + Hashable");
    match ty {
        Some(ParsedType::TraitBounds(bounds)) => {
            assert_eq!(bounds.len(), 2, "Expected 2 trait bounds");
            let ids = arena.get_parsed_type_list(bounds);
            // Both should be Named types
            assert!(
                matches!(arena.get_parsed_type(ids[0]), ParsedType::Named { .. }),
                "expected Named, got {:?}",
                arena.get_parsed_type(ids[0])
            );
            assert!(
                matches!(arena.get_parsed_type(ids[1]), ParsedType::Named { .. }),
                "expected Named, got {:?}",
                arena.get_parsed_type(ids[1])
            );
        }
        _ => panic!("expected TraitBounds, got {ty:?}"),
    }
}

#[test]
fn test_parse_trait_bounds_three() {
    // Printable + Hashable + Clone — three bounded trait object
    let (ty, arena) = parse_type_with_arena("Printable + Hashable + Clone");
    match ty {
        Some(ParsedType::TraitBounds(bounds)) => {
            assert_eq!(bounds.len(), 3, "Expected 3 trait bounds");
            let ids = arena.get_parsed_type_list(bounds);
            for (i, id) in ids.iter().enumerate() {
                assert!(
                    matches!(arena.get_parsed_type(*id), ParsedType::Named { .. }),
                    "bound {i} expected Named, got {:?}",
                    arena.get_parsed_type(*id)
                );
            }
        }
        _ => panic!("expected TraitBounds, got {ty:?}"),
    }
}

#[test]
fn test_single_trait_not_bounds() {
    // Single trait name should be Named, not TraitBounds
    let (ty, _) = parse_type_with_arena("Printable");
    assert!(
        matches!(ty, Some(ParsedType::Named { .. })),
        "single trait should be Named, got {ty:?}"
    );
}

#[test]
fn test_trait_bounds_in_list() {
    // [Printable + Hashable] — bounded trait object inside list type
    let (ty, arena) = parse_type_with_arena("[Printable + Hashable]");
    match ty {
        Some(ParsedType::List(inner_id)) => {
            let inner = arena.get_parsed_type(inner_id);
            assert!(
                matches!(inner, ParsedType::TraitBounds(_)),
                "expected TraitBounds inside list, got {inner:?}"
            );
        }
        _ => panic!("expected List, got {ty:?}"),
    }
}

#[test]
fn test_trait_bounds_preserves_names() {
    // Verify the actual trait names are correct
    let interner = StringInterner::new();
    let full_source = "@test () -> Printable + Hashable = 0;";
    let tokens = ori_lexer::lex(full_source, &interner);
    let mut parser = Parser::new(&tokens, &interner);

    parser.cursor.advance(); // @
    parser.cursor.advance(); // test
    parser.cursor.advance(); // (
    parser.cursor.advance(); // )
    parser.cursor.advance(); // ->

    let ty = parser.parse_type();
    let arena = parser.take_arena();

    match ty {
        Some(ParsedType::TraitBounds(bounds)) => {
            assert_eq!(bounds.len(), 2);
            let ids = arena.get_parsed_type_list(bounds);
            match arena.get_parsed_type(ids[0]) {
                ParsedType::Named { name, type_args } => {
                    assert_eq!(interner.lookup(*name), "Printable");
                    assert!(type_args.is_empty());
                }
                other => panic!("expected Named, got {other:?}"),
            }
            match arena.get_parsed_type(ids[1]) {
                ParsedType::Named { name, type_args } => {
                    assert_eq!(interner.lookup(*name), "Hashable");
                    assert!(type_args.is_empty());
                }
                other => panic!("expected Named, got {other:?}"),
            }
        }
        _ => panic!("expected TraitBounds, got {ty:?}"),
    }
}
