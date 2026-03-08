use ori_registry::{ReturnTag, TypeProjection, TypeTag};

use crate::{Idx, Pool, Tag};

use super::{return_tag_to_idx, tag_to_type_tag};

// Helper: create a minimal InferEngine for testing return_tag_to_idx.
fn with_engine(f: impl FnOnce(&mut crate::InferEngine<'_>)) {
    let mut pool = Pool::new();
    let mut engine = crate::InferEngine::new(&mut pool);
    f(&mut engine);
}

// ── tag_to_type_tag tests ──

/// Every `Tag` with builtin methods maps to the correct `TypeTag`.
#[test]
fn builtin_tags_map_correctly() {
    let cases: &[(Tag, TypeTag)] = &[
        (Tag::Int, TypeTag::Int),
        (Tag::Float, TypeTag::Float),
        (Tag::Bool, TypeTag::Bool),
        (Tag::Str, TypeTag::Str),
        (Tag::Char, TypeTag::Char),
        (Tag::Byte, TypeTag::Byte),
        (Tag::Duration, TypeTag::Duration),
        (Tag::Size, TypeTag::Size),
        (Tag::Ordering, TypeTag::Ordering),
        (Tag::Error, TypeTag::Error),
        (Tag::List, TypeTag::List),
        (Tag::Option, TypeTag::Option),
        (Tag::Result, TypeTag::Result),
        (Tag::Map, TypeTag::Map),
        (Tag::Set, TypeTag::Set),
        (Tag::Channel, TypeTag::Channel),
        (Tag::Range, TypeTag::Range),
        (Tag::Tuple, TypeTag::Tuple),
        (Tag::Iterator, TypeTag::Iterator),
        (Tag::DoubleEndedIterator, TypeTag::DoubleEndedIterator),
    ];

    for &(tag, expected) in cases {
        assert_eq!(
            tag_to_type_tag(tag),
            Some(expected),
            "Tag::{tag:?} should map to TypeTag::{expected:?}",
        );
    }
}

/// Non-builtin tags return None — these go through trait/impl dispatch.
#[test]
fn non_builtin_tags_return_none() {
    let non_builtin = [
        Tag::Named,
        Tag::Applied,
        Tag::Alias,
        Tag::Struct,
        Tag::Enum,
        Tag::Unit,
        Tag::Never,
        Tag::Var,
        Tag::BoundVar,
        Tag::RigidVar,
        Tag::Function,
        Tag::Scheme,
        Tag::Borrowed,
        Tag::Projection,
        Tag::ModuleNs,
        Tag::Infer,
        Tag::SelfType,
    ];

    for tag in non_builtin {
        assert_eq!(
            tag_to_type_tag(tag),
            None,
            "Tag::{tag:?} should return None (not a builtin registry type)",
        );
    }
}

/// Verifies exhaustive coverage of all 20 builtin Tag variants.
#[test]
fn all_tag_variants_covered() {
    let builtin_count = [
        Tag::Int,
        Tag::Float,
        Tag::Bool,
        Tag::Str,
        Tag::Char,
        Tag::Byte,
        Tag::Duration,
        Tag::Size,
        Tag::Ordering,
        Tag::Error,
        Tag::List,
        Tag::Option,
        Tag::Result,
        Tag::Map,
        Tag::Set,
        Tag::Channel,
        Tag::Range,
        Tag::Tuple,
        Tag::Iterator,
        Tag::DoubleEndedIterator,
    ]
    .iter()
    .filter(|t| tag_to_type_tag(**t).is_some())
    .count();

    assert_eq!(
        builtin_count, 20,
        "Expected 20 builtin tags to map to TypeTag"
    );
}

// ── return_tag_to_idx: primitive/concrete tests ──

#[test]
fn concrete_primitives_return_fixed_idx() {
    with_engine(|engine| {
        let cases: &[(TypeTag, Idx)] = &[
            (TypeTag::Int, Idx::INT),
            (TypeTag::Float, Idx::FLOAT),
            (TypeTag::Bool, Idx::BOOL),
            (TypeTag::Str, Idx::STR),
            (TypeTag::Char, Idx::CHAR),
            (TypeTag::Byte, Idx::BYTE),
            (TypeTag::Unit, Idx::UNIT),
            (TypeTag::Ordering, Idx::ORDERING),
            (TypeTag::Duration, Idx::DURATION),
            (TypeTag::Size, Idx::SIZE),
            (TypeTag::Error, Idx::ERROR),
        ];

        for &(type_tag, expected) in cases {
            let result = return_tag_to_idx(engine, Idx::INT, ReturnTag::Concrete(type_tag));
            assert_eq!(
                result, expected,
                "Concrete({type_tag:?}) should be {expected:?}"
            );
        }
    });
}

#[test]
fn self_type_returns_receiver() {
    with_engine(|engine| {
        let result = return_tag_to_idx(engine, Idx::STR, ReturnTag::SelfType);
        assert_eq!(result, Idx::STR);

        let list_int = engine.pool_mut().list(Idx::INT);
        let result = return_tag_to_idx(engine, list_int, ReturnTag::SelfType);
        assert_eq!(result, list_int);
    });
}

#[test]
fn unit_returns_unit_idx() {
    with_engine(|engine| {
        let result = return_tag_to_idx(engine, Idx::INT, ReturnTag::Unit);
        assert_eq!(result, Idx::UNIT);
    });
}

#[test]
fn fresh_returns_type_variable() {
    with_engine(|engine| {
        let result = return_tag_to_idx(engine, Idx::INT, ReturnTag::Fresh);
        assert_eq!(engine.pool().tag(result), Tag::Var);
    });
}

// ── return_tag_to_idx: projection tests ──

#[test]
fn option_of_element_on_list() {
    with_engine(|engine| {
        let list_int = engine.pool_mut().list(Idx::INT);
        let result = return_tag_to_idx(
            engine,
            list_int,
            ReturnTag::OptionOf(TypeProjection::Element),
        );
        assert_eq!(engine.pool().tag(result), Tag::Option);
        assert_eq!(engine.pool().option_inner(result), Idx::INT);
    });
}

#[test]
fn list_of_element_on_set() {
    with_engine(|engine| {
        let set_str = engine.pool_mut().set(Idx::STR);
        let result = return_tag_to_idx(engine, set_str, ReturnTag::ListOf(TypeProjection::Element));
        assert_eq!(engine.pool().tag(result), Tag::List);
        assert_eq!(engine.pool().list_elem(result), Idx::STR);
    });
}

#[test]
fn iterator_of_element_on_list() {
    with_engine(|engine| {
        let list_int = engine.pool_mut().list(Idx::INT);
        let result = return_tag_to_idx(
            engine,
            list_int,
            ReturnTag::IteratorOf(TypeProjection::Element),
        );
        assert_eq!(engine.pool().tag(result), Tag::Iterator);
        assert_eq!(engine.pool().iterator_elem(result), Idx::INT);
    });
}

#[test]
fn dei_of_element_on_list() {
    with_engine(|engine| {
        let list_str = engine.pool_mut().list(Idx::STR);
        let result = return_tag_to_idx(
            engine,
            list_str,
            ReturnTag::DoubleEndedIteratorOf(TypeProjection::Element),
        );
        assert_eq!(engine.pool().tag(result), Tag::DoubleEndedIterator);
        assert_eq!(engine.pool().iterator_elem(result), Idx::STR);
    });
}

#[test]
fn element_type_extracts_list_elem() {
    with_engine(|engine| {
        let list_int = engine.pool_mut().list(Idx::INT);
        let result = return_tag_to_idx(engine, list_int, ReturnTag::ElementType);
        assert_eq!(result, Idx::INT);
    });
}

#[test]
fn key_type_extracts_map_key() {
    with_engine(|engine| {
        let map = engine.pool_mut().map(Idx::STR, Idx::INT);
        let result = return_tag_to_idx(engine, map, ReturnTag::KeyType);
        assert_eq!(result, Idx::STR);
    });
}

#[test]
fn value_type_extracts_map_value() {
    with_engine(|engine| {
        let map = engine.pool_mut().map(Idx::STR, Idx::INT);
        let result = return_tag_to_idx(engine, map, ReturnTag::ValueType);
        assert_eq!(result, Idx::INT);
    });
}

#[test]
fn ok_type_extracts_result_ok() {
    with_engine(|engine| {
        let res = engine.pool_mut().result(Idx::STR, Idx::ERROR);
        let result = return_tag_to_idx(engine, res, ReturnTag::OkType);
        assert_eq!(result, Idx::STR);
    });
}

#[test]
fn err_type_extracts_result_err() {
    with_engine(|engine| {
        let res = engine.pool_mut().result(Idx::STR, Idx::ERROR);
        let result = return_tag_to_idx(engine, res, ReturnTag::ErrType);
        assert_eq!(result, Idx::ERROR);
    });
}

// ── return_tag_to_idx: fixed-inner wrappers ──

#[test]
fn list_of_byte_fixed_inner() {
    with_engine(|engine| {
        let result = return_tag_to_idx(engine, Idx::STR, ReturnTag::List(TypeTag::Byte));
        assert_eq!(engine.pool().tag(result), Tag::List);
        assert_eq!(engine.pool().list_elem(result), Idx::BYTE);
    });
}

#[test]
fn option_of_int_fixed_inner() {
    with_engine(|engine| {
        let result = return_tag_to_idx(engine, Idx::INT, ReturnTag::Option(TypeTag::Int));
        assert_eq!(engine.pool().tag(result), Tag::Option);
        assert_eq!(engine.pool().option_inner(result), Idx::INT);
    });
}

#[test]
fn dei_of_char_fixed_inner() {
    with_engine(|engine| {
        let result = return_tag_to_idx(
            engine,
            Idx::STR,
            ReturnTag::DoubleEndedIterator(TypeTag::Char),
        );
        assert_eq!(engine.pool().tag(result), Tag::DoubleEndedIterator);
        assert_eq!(engine.pool().iterator_elem(result), Idx::CHAR);
    });
}

// ── return_tag_to_idx: composite returns ──

#[test]
fn next_result_on_iterator() {
    with_engine(|engine| {
        let iter_int = engine.pool_mut().iterator(Idx::INT);
        let result = return_tag_to_idx(engine, iter_int, ReturnTag::NextResult);
        // Should be (Option<int>, Iterator<int>)
        assert_eq!(engine.pool().tag(result), Tag::Tuple);
        let elems = engine.pool().tuple_elems(result);
        assert_eq!(elems.len(), 2);
        // First element: Option<int>
        assert_eq!(engine.pool().tag(elems[0]), Tag::Option);
        assert_eq!(engine.pool().option_inner(elems[0]), Idx::INT);
        // Second element: Iterator<int> (same as receiver)
        assert_eq!(elems[1], iter_int);
    });
}

#[test]
fn result_of_projection_fresh_on_str() {
    with_engine(|engine| {
        let result = return_tag_to_idx(
            engine,
            Idx::STR,
            ReturnTag::ResultOfProjectionFresh(TypeProjection::Fixed(TypeTag::Str)),
        );
        assert_eq!(engine.pool().tag(result), Tag::Result);
        assert_eq!(engine.pool().result_ok(result), Idx::STR);
        let err_ty = engine.pool().result_err(result);
        assert_eq!(engine.pool().tag(err_ty), Tag::Var);
    });
}

#[test]
fn map_iterator_on_map() {
    with_engine(|engine| {
        let map = engine.pool_mut().map(Idx::STR, Idx::INT);
        let result = return_tag_to_idx(engine, map, ReturnTag::MapIterator);
        // Should be Iterator<(str, int)>
        assert_eq!(engine.pool().tag(result), Tag::Iterator);
        let pair = engine.pool().iterator_elem(result);
        assert_eq!(engine.pool().tag(pair), Tag::Tuple);
        let pair_elems = engine.pool().tuple_elems(pair);
        assert_eq!(pair_elems, &[Idx::STR, Idx::INT]);
    });
}

#[test]
fn list_key_value_on_map() {
    with_engine(|engine| {
        let map = engine.pool_mut().map(Idx::STR, Idx::INT);
        let result = return_tag_to_idx(engine, map, ReturnTag::ListKeyValue);
        // Should be [(str, int)]
        assert_eq!(engine.pool().tag(result), Tag::List);
        let pair = engine.pool().list_elem(result);
        assert_eq!(engine.pool().tag(pair), Tag::Tuple);
        let pair_elems = engine.pool().tuple_elems(pair);
        assert_eq!(pair_elems, &[Idx::STR, Idx::INT]);
    });
}

#[test]
fn list_of_tuple_int_element_on_list() {
    with_engine(|engine| {
        let list_str = engine.pool_mut().list(Idx::STR);
        let result = return_tag_to_idx(engine, list_str, ReturnTag::ListOfTupleIntElement);
        // Should be [(int, str)]
        assert_eq!(engine.pool().tag(result), Tag::List);
        let pair = engine.pool().list_elem(result);
        assert_eq!(engine.pool().tag(pair), Tag::Tuple);
        let pair_elems = engine.pool().tuple_elems(pair);
        assert_eq!(pair_elems, &[Idx::INT, Idx::STR]);
    });
}

#[test]
fn iterator_of_tuple_int_element_on_iterator() {
    with_engine(|engine| {
        let iter_str = engine.pool_mut().iterator(Idx::STR);
        let result = return_tag_to_idx(engine, iter_str, ReturnTag::IteratorOfTupleIntElement);
        // Should be Iterator<(int, str)>
        assert_eq!(engine.pool().tag(result), Tag::Iterator);
        let pair = engine.pool().iterator_elem(result);
        assert_eq!(engine.pool().tag(pair), Tag::Tuple);
        let pair_elems = engine.pool().tuple_elems(pair);
        assert_eq!(pair_elems, &[Idx::INT, Idx::STR]);
    });
}
