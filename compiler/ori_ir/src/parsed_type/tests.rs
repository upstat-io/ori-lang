use super::*;
use crate::ExprArena;

#[test]
fn test_primitive() {
    let ty = ParsedType::primitive(TypeId::INT);
    assert!(ty.is_primitive());
    assert!(!ty.is_function());
}

#[test]
fn test_named() {
    let name = Name::new(0, 1); // dummy name
    let ty = ParsedType::named(name);
    assert!(!ty.is_primitive());
    match ty {
        ParsedType::Named { name: n, type_args } => {
            assert_eq!(n, name);
            assert!(type_args.is_empty());
        }
        _ => panic!("expected Named"),
    }
}

#[test]
fn test_list() {
    let mut arena = ExprArena::new();
    let elem_id = arena.alloc_parsed_type(ParsedType::primitive(TypeId::INT));
    let ty = ParsedType::list(elem_id);
    match ty {
        ParsedType::List(id) => {
            assert_eq!(
                *arena.get_parsed_type(id),
                ParsedType::primitive(TypeId::INT)
            );
        }
        _ => panic!("expected List"),
    }
}

#[test]
fn test_function() {
    let mut arena = ExprArena::new();
    let param_id = arena.alloc_parsed_type(ParsedType::primitive(TypeId::INT));
    let ret_id = arena.alloc_parsed_type(ParsedType::primitive(TypeId::BOOL));
    let params = arena.alloc_parsed_type_list([param_id]);
    let ty = ParsedType::function(params, ret_id);
    assert!(ty.is_function());
    match ty {
        ParsedType::Function { params, ret } => {
            assert_eq!(params.len(), 1);
            assert_eq!(
                *arena.get_parsed_type(ret),
                ParsedType::primitive(TypeId::BOOL)
            );
        }
        _ => panic!("expected Function"),
    }
}

#[test]
fn test_unit() {
    let ty = ParsedType::unit();
    match ty {
        ParsedType::Tuple(elems) => {
            assert!(elems.is_empty());
        }
        _ => panic!("expected Tuple"),
    }
}

#[test]
fn test_equality() {
    let ty1 = ParsedType::primitive(TypeId::INT);
    let ty2 = ParsedType::primitive(TypeId::INT);
    let ty3 = ParsedType::primitive(TypeId::FLOAT);

    assert_eq!(ty1, ty2);
    assert_ne!(ty1, ty3);
}

#[test]
fn test_nested_generic() {
    // Option<Result<int, str>>
    let mut arena = ExprArena::new();
    let name_option = Name::new(0, 1);
    let name_result = Name::new(0, 2);

    // Create inner type: Result<int, str>
    let int_id = arena.alloc_parsed_type(ParsedType::primitive(TypeId::INT));
    let str_id = arena.alloc_parsed_type(ParsedType::primitive(TypeId::STR));
    let inner_args = arena.alloc_parsed_type_list([int_id, str_id]);
    let inner = ParsedType::named_with_args(name_result, inner_args);

    // Create outer type: Option<Result<int, str>>
    let inner_id = arena.alloc_parsed_type(inner);
    let outer_args = arena.alloc_parsed_type_list([inner_id]);
    let ty = ParsedType::named_with_args(name_option, outer_args);

    match ty {
        ParsedType::Named { name, type_args } => {
            assert_eq!(name, name_option);
            assert_eq!(type_args.len(), 1);
            let inner_ids = arena.get_parsed_type_list(type_args);
            match arena.get_parsed_type(inner_ids[0]) {
                ParsedType::Named {
                    name,
                    type_args: inner_args,
                } => {
                    assert_eq!(*name, name_result);
                    assert_eq!(inner_args.len(), 2);
                }
                _ => panic!("expected Named"),
            }
        }
        _ => panic!("expected Named"),
    }
}

#[test]
fn test_map_type() {
    let mut arena = ExprArena::new();
    let key_id = arena.alloc_parsed_type(ParsedType::primitive(TypeId::STR));
    let value_id = arena.alloc_parsed_type(ParsedType::primitive(TypeId::INT));
    let ty = ParsedType::map(key_id, value_id);
    match ty {
        ParsedType::Map { key, value } => {
            assert_eq!(
                *arena.get_parsed_type(key),
                ParsedType::primitive(TypeId::STR)
            );
            assert_eq!(
                *arena.get_parsed_type(value),
                ParsedType::primitive(TypeId::INT)
            );
        }
        _ => panic!("expected Map"),
    }
}

#[test]
fn test_associated_type() {
    let mut arena = ExprArena::new();
    let base_id = arena.alloc_parsed_type(ParsedType::SelfType);
    let assoc_name = Name::new(0, 5);
    let ty = ParsedType::associated_type(base_id, assoc_name);
    match ty {
        ParsedType::AssociatedType {
            base,
            assoc_name: name,
        } => {
            assert_eq!(*arena.get_parsed_type(base), ParsedType::SelfType);
            assert_eq!(name, assoc_name);
        }
        _ => panic!("expected AssociatedType"),
    }
}

#[test]
fn test_tuple_type() {
    let mut arena = ExprArena::new();
    let int_id = arena.alloc_parsed_type(ParsedType::primitive(TypeId::INT));
    let bool_id = arena.alloc_parsed_type(ParsedType::primitive(TypeId::BOOL));
    let elems = arena.alloc_parsed_type_list([int_id, bool_id]);
    let ty = ParsedType::tuple(elems);
    match ty {
        ParsedType::Tuple(range) => {
            assert_eq!(range.len(), 2);
            let ids = arena.get_parsed_type_list(range);
            assert_eq!(
                *arena.get_parsed_type(ids[0]),
                ParsedType::primitive(TypeId::INT)
            );
            assert_eq!(
                *arena.get_parsed_type(ids[1]),
                ParsedType::primitive(TypeId::BOOL)
            );
        }
        _ => panic!("expected Tuple"),
    }
}
