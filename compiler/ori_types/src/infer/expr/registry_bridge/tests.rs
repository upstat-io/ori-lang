use ori_registry::{ReturnTag, TypeProjection, TypeTag};

use crate::{Idx, Pool, Tag};

use super::{
    registry_satisfies_trait, registry_type_satisfies_trait, return_tag_to_idx, tag_to_type_tag,
};

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

// ── binary_op_strategy: semantic pin tests ──
// These verify that the registry bridge correctly maps BinaryOp to OpDefs
// for each primitive type. This is the permanent guard that ensures the
// type checker's registry-based validation matches expected behavior.

use ori_ir::BinaryOp;
use ori_registry::OpStrategy;

use super::{binary_op_strategy, is_binary_op_supported};

/// Int supports all arithmetic and bitwise operators.
#[test]
fn int_arithmetic_ops_supported() {
    let arithmetic = [
        BinaryOp::Add,
        BinaryOp::Sub,
        BinaryOp::Mul,
        BinaryOp::Div,
        BinaryOp::Mod,
        BinaryOp::FloorDiv,
    ];
    for op in arithmetic {
        assert_eq!(
            is_binary_op_supported(Tag::Int, op),
            Some(true),
            "int should support {op:?}",
        );
    }
}

/// Int supports all bitwise operators.
#[test]
fn int_bitwise_ops_supported() {
    let bitwise = [
        BinaryOp::BitAnd,
        BinaryOp::BitOr,
        BinaryOp::BitXor,
        BinaryOp::Shl,
        BinaryOp::Shr,
    ];
    for op in bitwise {
        assert_eq!(
            is_binary_op_supported(Tag::Int, op),
            Some(true),
            "int should support {op:?}",
        );
    }
}

/// Float supports arithmetic but not bitwise.
#[test]
fn float_arithmetic_supported_bitwise_not() {
    assert_eq!(
        is_binary_op_supported(Tag::Float, BinaryOp::Add),
        Some(true)
    );
    assert_eq!(
        is_binary_op_supported(Tag::Float, BinaryOp::BitAnd),
        Some(false)
    );
    assert_eq!(
        is_binary_op_supported(Tag::Float, BinaryOp::Shl),
        Some(false)
    );
}

/// Bool does NOT support arithmetic.
#[test]
fn bool_arithmetic_not_supported() {
    assert_eq!(
        is_binary_op_supported(Tag::Bool, BinaryOp::Add),
        Some(false)
    );
    assert_eq!(
        is_binary_op_supported(Tag::Bool, BinaryOp::Sub),
        Some(false)
    );
    assert_eq!(
        is_binary_op_supported(Tag::Bool, BinaryOp::Mul),
        Some(false)
    );
}

/// Duration supports add/sub/mul/div/rem but NOT `floor_div`.
#[test]
fn duration_operator_support() {
    assert_eq!(
        is_binary_op_supported(Tag::Duration, BinaryOp::Add),
        Some(true)
    );
    assert_eq!(
        is_binary_op_supported(Tag::Duration, BinaryOp::Sub),
        Some(true)
    );
    assert_eq!(
        is_binary_op_supported(Tag::Duration, BinaryOp::Mul),
        Some(true)
    );
    assert_eq!(
        is_binary_op_supported(Tag::Duration, BinaryOp::FloorDiv),
        Some(false)
    );
    assert_eq!(
        is_binary_op_supported(Tag::Duration, BinaryOp::BitAnd),
        Some(false)
    );
}

/// Size supports add/sub/mul/div/rem but NOT neg or `floor_div`.
#[test]
fn size_operator_support() {
    assert_eq!(is_binary_op_supported(Tag::Size, BinaryOp::Add), Some(true));
    assert_eq!(is_binary_op_supported(Tag::Size, BinaryOp::Mul), Some(true));
    assert_eq!(
        is_binary_op_supported(Tag::Size, BinaryOp::FloorDiv),
        Some(false)
    );
    assert_eq!(
        is_binary_op_supported(Tag::Size, BinaryOp::BitAnd),
        Some(false)
    );
}

/// Str supports add (concatenation) but nothing else arithmetic.
#[test]
fn str_only_add_supported() {
    assert_eq!(is_binary_op_supported(Tag::Str, BinaryOp::Add), Some(true));
    assert_eq!(is_binary_op_supported(Tag::Str, BinaryOp::Sub), Some(false));
    assert_eq!(is_binary_op_supported(Tag::Str, BinaryOp::Mul), Some(false));
}

/// List supports add (concatenation) but nothing else.
#[test]
fn list_only_add_supported() {
    assert_eq!(is_binary_op_supported(Tag::List, BinaryOp::Add), Some(true));
    assert_eq!(
        is_binary_op_supported(Tag::List, BinaryOp::Sub),
        Some(false)
    );
}

/// `MatMul`, `And`, `Or`, `Range`, `Coalesce` return `None` (not tracked by `OpDefs`).
#[test]
fn non_opdefs_operators_return_none() {
    let non_tracked = [
        BinaryOp::MatMul,
        BinaryOp::And,
        BinaryOp::Or,
        BinaryOp::Range,
        BinaryOp::RangeInclusive,
        BinaryOp::Coalesce,
    ];
    for op in non_tracked {
        assert_eq!(
            binary_op_strategy(Tag::Int, op),
            None,
            "{op:?} should not be tracked by OpDefs",
        );
    }
}

/// Non-builtin tags return None.
#[test]
fn non_builtin_tag_returns_none_for_ops() {
    assert_eq!(is_binary_op_supported(Tag::Struct, BinaryOp::Add), None);
    assert_eq!(is_binary_op_supported(Tag::Enum, BinaryOp::Eq), None);
    assert_eq!(is_binary_op_supported(Tag::Named, BinaryOp::Sub), None);
}

/// Int add strategy is `IntInstr` (specific variant check).
#[test]
fn int_add_strategy_is_int_instr() {
    assert_eq!(
        binary_op_strategy(Tag::Int, BinaryOp::Add),
        Some(OpStrategy::IntInstr)
    );
}

/// Str add strategy is a `RuntimeCall` (specific variant check).
#[test]
fn str_add_strategy_is_runtime_call() {
    match binary_op_strategy(Tag::Str, BinaryOp::Add) {
        Some(OpStrategy::RuntimeCall { fn_name, .. }) => {
            assert_eq!(fn_name, "ori_str_concat");
        }
        other => panic!("expected RuntimeCall, got {other:?}"),
    }
}

// ── unary_op_strategy: semantic pin tests ──

use ori_ir::UnaryOp;

use super::{is_unary_op_supported, unary_op_strategy};

/// Int supports negation and bitwise NOT but not logical NOT.
#[test]
fn int_unary_support() {
    assert_eq!(is_unary_op_supported(Tag::Int, UnaryOp::Neg), Some(true));
    assert_eq!(is_unary_op_supported(Tag::Int, UnaryOp::BitNot), Some(true));
    assert_eq!(is_unary_op_supported(Tag::Int, UnaryOp::Not), Some(false));
}

/// Float supports negation only.
#[test]
fn float_unary_support() {
    assert_eq!(is_unary_op_supported(Tag::Float, UnaryOp::Neg), Some(true));
    assert_eq!(is_unary_op_supported(Tag::Float, UnaryOp::Not), Some(false));
    assert_eq!(
        is_unary_op_supported(Tag::Float, UnaryOp::BitNot),
        Some(false)
    );
}

/// Bool supports logical NOT only.
#[test]
fn bool_unary_support() {
    assert_eq!(is_unary_op_supported(Tag::Bool, UnaryOp::Not), Some(true));
    assert_eq!(is_unary_op_supported(Tag::Bool, UnaryOp::Neg), Some(false));
    assert_eq!(
        is_unary_op_supported(Tag::Bool, UnaryOp::BitNot),
        Some(false)
    );
}

/// Duration supports negation but not NOT or `BitNot`.
#[test]
fn duration_unary_support() {
    assert_eq!(
        is_unary_op_supported(Tag::Duration, UnaryOp::Neg),
        Some(true)
    );
    assert_eq!(
        is_unary_op_supported(Tag::Duration, UnaryOp::Not),
        Some(false)
    );
}

/// Size does NOT support negation (non-negative by design).
#[test]
fn size_unary_support() {
    assert_eq!(is_unary_op_supported(Tag::Size, UnaryOp::Neg), Some(false));
}

/// `Try` operator returns `None` (dedicated dispatch, not in `OpDefs`).
#[test]
fn try_op_returns_none() {
    assert_eq!(unary_op_strategy(Tag::Int, UnaryOp::Try), None);
    assert_eq!(unary_op_strategy(Tag::Option, UnaryOp::Try), None);
}

/// Non-builtin tags return `None` for unary ops.
#[test]
fn non_builtin_unary_returns_none() {
    assert_eq!(is_unary_op_supported(Tag::Struct, UnaryOp::Neg), None);
    assert_eq!(is_unary_op_supported(Tag::Enum, UnaryOp::Not), None);
}

// ── registry_satisfies_trait: equivalence tests ──
// These verify the bridge produces identical results to the old
// hardcoded arrays in traits.rs for every (type, trait) combination.

/// All trait names that appear in the old hardcoded arrays.
const ALL_TRAIT_NAMES: &[&str] = &[
    "Eq",
    "Comparable",
    "Clone",
    "Hashable",
    "Default",
    "Printable",
    "Debug",
    "Add",
    "Sub",
    "Mul",
    "Div",
    "FloorDiv",
    "Rem",
    "Neg",
    "Not",
    "BitAnd",
    "BitOr",
    "BitXor",
    "BitNot",
    "Shl",
    "Shr",
    "Sendable",
    "Len",
    "IsEmpty",
    "Iterable",
    "Iterator",
    "DoubleEndedIterator",
];

/// Expected trait sets per type — the equivalence truth table.
#[expect(clippy::too_many_lines, reason = "exhaustive type × trait truth table")]
fn expected_traits(tag: TypeTag) -> &'static [&'static str] {
    match tag {
        TypeTag::Int => &[
            "Eq",
            "Comparable",
            "Clone",
            "Hashable",
            "Default",
            "Printable",
            "Debug",
            "Add",
            "Sub",
            "Mul",
            "Div",
            "FloorDiv",
            "Rem",
            "Neg",
            "BitAnd",
            "BitOr",
            "BitXor",
            "BitNot",
            "Shl",
            "Shr",
        ],
        TypeTag::Float => &[
            "Eq",
            "Comparable",
            "Clone",
            "Hashable",
            "Default",
            "Printable",
            "Debug",
            "Add",
            "Sub",
            "Mul",
            "Div",
            "Rem",
            "Neg",
        ],
        TypeTag::Bool => &[
            "Eq",
            "Comparable",
            "Clone",
            "Hashable",
            "Default",
            "Printable",
            "Debug",
            "Not",
        ],
        TypeTag::Str => &[
            "Eq",
            "Comparable",
            "Clone",
            "Hashable",
            "Default",
            "Printable",
            "Debug",
            "Add",
            "Len",
            "IsEmpty",
            "Iterable",
        ],
        TypeTag::Char | TypeTag::Ordering | TypeTag::Result => &[
            "Eq",
            "Comparable",
            "Clone",
            "Hashable",
            "Printable",
            "Debug",
        ],
        TypeTag::Byte => &[
            "Eq",
            "Comparable",
            "Clone",
            "Hashable",
            "Printable",
            "Debug",
            "Add",
            "Sub",
            "Mul",
            "Div",
            "Rem",
            "BitAnd",
            "BitOr",
            "BitXor",
            "BitNot",
            "Shl",
            "Shr",
        ],
        TypeTag::Unit => &["Eq", "Comparable", "Hashable", "Clone", "Default", "Debug"],
        TypeTag::Duration => &[
            "Eq",
            "Comparable",
            "Clone",
            "Hashable",
            "Default",
            "Printable",
            "Debug",
            "Sendable",
            "Add",
            "Sub",
            "Mul",
            "Div",
            "Rem",
            "Neg",
        ],
        TypeTag::Size => &[
            "Eq",
            "Comparable",
            "Clone",
            "Hashable",
            "Default",
            "Printable",
            "Debug",
            "Sendable",
            "Add",
            "Sub",
            "Mul",
            "Div",
            "Rem",
        ],
        // Compound types — derived from registry TypeDef
        TypeTag::List => &[
            "Eq",
            "Comparable",
            "Clone",
            "Hashable",
            "Printable",
            "Debug",
            "Add",
            "Len",
            "IsEmpty",
            "Iterable",
        ],
        TypeTag::Map | TypeTag::Set => &[
            "Eq",
            "Clone",
            "Hashable",
            "Printable",
            "Debug",
            "Len",
            "IsEmpty",
            "Iterable",
        ],
        TypeTag::Option => &[
            "Eq",
            "Comparable",
            "Clone",
            "Hashable",
            "Printable",
            "Default",
            "Debug",
            "Iterable",
        ],
        TypeTag::Tuple => &[
            "Eq",
            "Comparable",
            "Clone",
            "Hashable",
            "Printable",
            "Debug",
            "Len",
        ],
        TypeTag::Range => &["Printable", "Len", "IsEmpty", "Iterable"],
        TypeTag::Iterator => &["Iterator"],
        TypeTag::DoubleEndedIterator => &["Iterator", "DoubleEndedIterator"],
        // Error has methods (clone, debug, to_str, etc.) — registry encodes them
        TypeTag::Error => &["Clone", "Printable", "Debug"],
        // Channel has Len/IsEmpty from trait_name on len/is_empty methods
        TypeTag::Channel => &["Len", "IsEmpty"],
        // Types with no trait satisfaction
        TypeTag::Never | TypeTag::Function => &[],
    }
}

/// Equivalence test: registry bridge matches expected trait sets for ALL
/// (`TypeTag`, `trait_name`) combinations.
#[test]
fn registry_bridge_matches_old_trait_arrays() {
    let type_tags = TypeTag::all();
    let mut checked = 0;
    let mut mismatches = Vec::new();

    for &type_tag in type_tags {
        let expected = expected_traits(type_tag);
        for &trait_name in ALL_TRAIT_NAMES {
            let bridge_result = registry_satisfies_trait(type_tag, trait_name);
            let old_result = expected.contains(&trait_name);
            if bridge_result != old_result {
                mismatches.push(format!(
                    "TypeTag::{type_tag:?} x \"{trait_name}\": bridge={bridge_result}, expected={old_result}"
                ));
            }
            checked += 1;
        }
    }

    assert!(
        mismatches.is_empty(),
        "Found {} mismatches:\n{}",
        mismatches.len(),
        mismatches.join("\n")
    );

    // Verify matrix completeness
    let expected_count = type_tags.len() * ALL_TRAIT_NAMES.len();
    assert_eq!(
        checked, expected_count,
        "Expected {expected_count} checks, got {checked}"
    );
}

/// Semantic pin: specific positive assertions per type.
#[test]
fn registry_satisfies_trait_positive_pins() {
    // Int: arithmetic + bitwise + comparison
    assert!(registry_satisfies_trait(TypeTag::Int, "BitNot"));
    assert!(registry_satisfies_trait(TypeTag::Int, "FloorDiv"));
    assert!(registry_satisfies_trait(TypeTag::Int, "Shl"));
    assert!(registry_satisfies_trait(TypeTag::Int, "Default"));

    // Str: Len + IsEmpty + Iterable + Add
    assert!(registry_satisfies_trait(TypeTag::Str, "Len"));
    assert!(registry_satisfies_trait(TypeTag::Str, "IsEmpty"));
    assert!(registry_satisfies_trait(TypeTag::Str, "Iterable"));
    assert!(registry_satisfies_trait(TypeTag::Str, "Add"));

    // Duration: Sendable + Neg
    assert!(registry_satisfies_trait(TypeTag::Duration, "Sendable"));
    assert!(registry_satisfies_trait(TypeTag::Duration, "Neg"));

    // Iterator: just "Iterator" meta-trait
    assert!(registry_satisfies_trait(TypeTag::Iterator, "Iterator"));

    // DEI: both Iterator and DoubleEndedIterator
    assert!(registry_satisfies_trait(
        TypeTag::DoubleEndedIterator,
        "Iterator"
    ));
    assert!(registry_satisfies_trait(
        TypeTag::DoubleEndedIterator,
        "DoubleEndedIterator"
    ));
}

/// Negative pin: specific assertions per type that traits are NOT satisfied.
#[test]
fn registry_satisfies_trait_negative_pins() {
    // Int: no Not (logical)
    assert!(!registry_satisfies_trait(TypeTag::Int, "Not"));
    // Bool: no Add
    assert!(!registry_satisfies_trait(TypeTag::Bool, "Add"));
    // Char: no Default
    assert!(!registry_satisfies_trait(TypeTag::Char, "Default"));
    // Byte: no Neg
    assert!(!registry_satisfies_trait(TypeTag::Byte, "Neg"));
    // Unit: no Printable
    assert!(!registry_satisfies_trait(TypeTag::Unit, "Printable"));
    // Iterator: not Clone, not Eq
    assert!(!registry_satisfies_trait(TypeTag::Iterator, "Clone"));
    assert!(!registry_satisfies_trait(TypeTag::Iterator, "Eq"));
    // Channel: no traits
    assert!(!registry_satisfies_trait(TypeTag::Channel, "Eq"));
    assert!(!registry_satisfies_trait(TypeTag::Channel, "Clone"));
    // Error: no Eq
    assert!(!registry_satisfies_trait(TypeTag::Error, "Eq"));
}

/// `registry_type_satisfies_trait` returns `None` for non-builtin tags.
#[test]
fn registry_type_satisfies_non_builtin_returns_none() {
    assert_eq!(registry_type_satisfies_trait(Tag::Named, "Eq"), None);
    assert_eq!(registry_type_satisfies_trait(Tag::Struct, "Clone"), None);
    assert_eq!(registry_type_satisfies_trait(Tag::Enum, "Hashable"), None);
    assert_eq!(
        registry_type_satisfies_trait(Tag::Function, "Printable"),
        None
    );
}

/// `registry_type_satisfies_trait` returns `Some` for builtin tags,
/// including Unit and Never which have no `TypeDef`.
#[test]
fn registry_type_satisfies_builtin_returns_some() {
    assert_eq!(registry_type_satisfies_trait(Tag::Int, "Add"), Some(true));
    assert_eq!(registry_type_satisfies_trait(Tag::Int, "Not"), Some(false));
    assert_eq!(
        registry_type_satisfies_trait(Tag::Unit, "Default"),
        Some(true)
    );
    assert_eq!(
        registry_type_satisfies_trait(Tag::Unit, "Printable"),
        Some(false)
    );
    assert_eq!(registry_type_satisfies_trait(Tag::Never, "Eq"), Some(false));
}
