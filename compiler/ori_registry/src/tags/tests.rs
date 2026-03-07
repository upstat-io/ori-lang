use super::*;
use std::collections::HashSet;

#[test]
fn type_tag_all_returns_correct_count() {
    assert_eq!(
        TypeTag::all().len(),
        23,
        "TypeTag::all() should return exactly 23 variants — update this \
         count and ALL_TYPE_TAGS if you add a new variant"
    );
}

#[test]
fn type_tag_name_returns_correct_ori_names() {
    let expected: &[(TypeTag, &str)] = &[
        (TypeTag::Int, "int"),
        (TypeTag::Float, "float"),
        (TypeTag::Bool, "bool"),
        (TypeTag::Char, "char"),
        (TypeTag::Byte, "byte"),
        (TypeTag::Unit, "void"),
        (TypeTag::Never, "Never"),
        (TypeTag::Duration, "Duration"),
        (TypeTag::Size, "Size"),
        (TypeTag::Ordering, "Ordering"),
        (TypeTag::Str, "str"),
        (TypeTag::Error, "Error"),
        (TypeTag::List, "List"),
        (TypeTag::Map, "Map"),
        (TypeTag::Set, "Set"),
        (TypeTag::Range, "Range"),
        (TypeTag::Tuple, "Tuple"),
        (TypeTag::Option, "Option"),
        (TypeTag::Result, "Result"),
        (TypeTag::Channel, "Channel"),
        (TypeTag::Function, "Function"),
        (TypeTag::Iterator, "Iterator"),
        (TypeTag::DoubleEndedIterator, "DoubleEndedIterator"),
    ];

    for &(tag, name) in expected {
        assert_eq!(tag.name(), name, "TypeTag::{tag:?}.name() mismatch");
    }

    // Verify we tested every variant
    assert_eq!(expected.len(), TypeTag::all().len());
}

#[test]
fn type_tag_no_duplicate_discriminants() {
    let mut seen = HashSet::new();
    for &tag in TypeTag::all() {
        let disc = tag as u8;
        assert!(
            seen.insert(disc),
            "Duplicate discriminant {disc} for TypeTag::{tag:?}"
        );
    }
}

#[test]
fn type_tag_base_type_dei_aliases_to_iterator() {
    assert_eq!(
        TypeTag::DoubleEndedIterator.base_type(),
        TypeTag::Iterator,
        "DoubleEndedIterator.base_type() should return Iterator"
    );
}

#[test]
fn type_tag_base_type_returns_self_for_non_dei() {
    for &tag in TypeTag::all() {
        if tag == TypeTag::DoubleEndedIterator {
            continue;
        }
        assert_eq!(
            tag.base_type(),
            tag,
            "TypeTag::{tag:?}.base_type() should return self"
        );
    }
}

#[test]
fn type_tag_is_primitive_covers_exactly_five() {
    let primitives: Vec<TypeTag> = TypeTag::all()
        .iter()
        .copied()
        .filter(|t| t.is_primitive())
        .collect();

    assert_eq!(
        primitives,
        vec![
            TypeTag::Int,
            TypeTag::Float,
            TypeTag::Bool,
            TypeTag::Char,
            TypeTag::Byte,
        ]
    );
}

#[test]
fn type_tag_is_generic_covers_parameterized_types() {
    let generics: Vec<TypeTag> = TypeTag::all()
        .iter()
        .copied()
        .filter(|t| t.is_generic())
        .collect();

    assert_eq!(
        generics,
        vec![
            TypeTag::List,
            TypeTag::Map,
            TypeTag::Set,
            TypeTag::Range,
            TypeTag::Tuple,
            TypeTag::Option,
            TypeTag::Result,
            TypeTag::Channel,
            TypeTag::Function,
            TypeTag::Iterator,
            TypeTag::DoubleEndedIterator,
        ]
    );
}

#[test]
fn type_tag_primitive_and_generic_are_disjoint() {
    for &tag in TypeTag::all() {
        assert!(
            !(tag.is_primitive() && tag.is_generic()),
            "TypeTag::{tag:?} cannot be both primitive and generic"
        );
    }
}

#[test]
fn type_tag_size_is_one_byte() {
    assert_eq!(
        std::mem::size_of::<TypeTag>(),
        1,
        "TypeTag must be 1 byte (#[repr(u8)])"
    );
}

// MemoryStrategy tests

#[test]
fn memory_strategy_size_is_one_byte() {
    assert_eq!(std::mem::size_of::<MemoryStrategy>(), 1);
}

#[test]
fn memory_strategy_variants_are_distinct() {
    assert_ne!(MemoryStrategy::Copy, MemoryStrategy::Arc);
    assert_ne!(MemoryStrategy::Copy, MemoryStrategy::Structural);
    assert_ne!(MemoryStrategy::Arc, MemoryStrategy::Structural);
}

#[test]
fn memory_strategy_is_const_constructible() {
    const COPY: MemoryStrategy = MemoryStrategy::Copy;
    const ARC: MemoryStrategy = MemoryStrategy::Arc;
    const STRUCTURAL: MemoryStrategy = MemoryStrategy::Structural;
    assert_eq!(COPY, MemoryStrategy::Copy);
    assert_eq!(ARC, MemoryStrategy::Arc);
    assert_eq!(STRUCTURAL, MemoryStrategy::Structural);
}

#[test]
fn each_registered_type_has_exactly_one_memory_strategy() {
    use crate::defs::BUILTIN_TYPES;

    for type_def in BUILTIN_TYPES {
        // Each TypeDef has exactly one MemoryStrategy — this is enforced
        // by the struct field being a single enum value. Verify it's a
        // known variant (not some future variant we haven't handled).
        let strategy = type_def.memory;
        assert!(
            strategy == MemoryStrategy::Copy
                || strategy == MemoryStrategy::Arc
                || strategy == MemoryStrategy::Structural,
            "type `{}` has unknown MemoryStrategy: {:?}",
            type_def.name,
            strategy
        );
    }

    // Verify expected strategies for registered types.
    let copy_types: Vec<&str> = BUILTIN_TYPES
        .iter()
        .filter(|t| t.memory == MemoryStrategy::Copy)
        .map(|t| t.name)
        .collect();
    let arc_types: Vec<&str> = BUILTIN_TYPES
        .iter()
        .filter(|t| t.memory == MemoryStrategy::Arc)
        .map(|t| t.name)
        .collect();

    let structural_types: Vec<&str> = BUILTIN_TYPES
        .iter()
        .filter(|t| t.memory == MemoryStrategy::Structural)
        .map(|t| t.name)
        .collect();

    assert_eq!(
        copy_types,
        vec!["int", "float", "bool", "char", "byte", "Duration", "Size", "Ordering", "Range"]
    );
    assert_eq!(
        arc_types,
        vec!["str", "Error", "List", "Map", "Set", "Channel", "Iterator"]
    );
    assert_eq!(structural_types, vec!["Tuple", "Option", "Result"]);
}

// Ownership tests

#[test]
fn ownership_size_is_one_byte() {
    assert_eq!(std::mem::size_of::<Ownership>(), 1);
}

#[test]
fn ownership_all_variants_distinct() {
    assert_ne!(Ownership::Borrow, Ownership::Owned);
    assert_ne!(Ownership::Owned, Ownership::Copy);
    assert_ne!(Ownership::Copy, Ownership::Borrow);
}

#[test]
fn ownership_is_const_constructible() {
    const B: Ownership = Ownership::Borrow;
    const O: Ownership = Ownership::Owned;
    const C: Ownership = Ownership::Copy;
    assert_eq!(B, Ownership::Borrow);
    assert_eq!(O, Ownership::Owned);
    assert_eq!(C, Ownership::Copy);
}

// OpStrategy tests

#[test]
fn op_strategy_size_matches_pointer_width() {
    let expected = if cfg!(target_pointer_width = "64") {
        24
    } else {
        12
    };
    assert_eq!(
        std::mem::size_of::<OpStrategy>(),
        expected,
        "OpStrategy size should match pointer width"
    );
}

#[test]
fn op_strategy_variants_equality() {
    assert_eq!(OpStrategy::IntInstr, OpStrategy::IntInstr);
    assert_eq!(OpStrategy::FloatInstr, OpStrategy::FloatInstr);
    assert_eq!(OpStrategy::UnsignedCmp, OpStrategy::UnsignedCmp);
    assert_eq!(OpStrategy::BoolLogic, OpStrategy::BoolLogic);
    assert_eq!(OpStrategy::Unsupported, OpStrategy::Unsupported);

    assert_ne!(OpStrategy::IntInstr, OpStrategy::FloatInstr);
    assert_ne!(OpStrategy::IntInstr, OpStrategy::Unsupported);
}

#[test]
fn op_strategy_runtime_call_equality() {
    let a = OpStrategy::RuntimeCall {
        fn_name: "ori_str_concat",
        returns_bool: false,
    };
    let b = OpStrategy::RuntimeCall {
        fn_name: "ori_str_concat",
        returns_bool: false,
    };
    let c = OpStrategy::RuntimeCall {
        fn_name: "ori_str_eq",
        returns_bool: true,
    };

    assert_eq!(a, b);
    assert_ne!(a, c);
}

#[test]
fn op_strategy_runtime_call_is_const_constructible() {
    const CONCAT: OpStrategy = OpStrategy::RuntimeCall {
        fn_name: "ori_str_concat",
        returns_bool: false,
    };
    const EQ: OpStrategy = OpStrategy::RuntimeCall {
        fn_name: "ori_str_eq",
        returns_bool: true,
    };
    // Const context verified by the const declarations above.
    assert_ne!(CONCAT, EQ);
}

#[test]
fn op_strategy_debug_output() {
    let s = format!("{:?}", OpStrategy::IntInstr);
    assert_eq!(s, "IntInstr");

    let s = format!(
        "{:?}",
        OpStrategy::RuntimeCall {
            fn_name: "ori_str_eq",
            returns_bool: true
        }
    );
    assert!(s.contains("ori_str_eq"));
    assert!(s.contains("true"));
}

#[test]
fn op_strategy_hash_consistency() {
    use std::hash::{Hash, Hasher};

    fn hash_of(v: &OpStrategy) -> u64 {
        let mut h = std::collections::hash_map::DefaultHasher::new();
        v.hash(&mut h);
        h.finish()
    }

    let a = OpStrategy::IntInstr;
    let b = OpStrategy::IntInstr;
    assert_eq!(hash_of(&a), hash_of(&b));

    // Different variants should (likely) have different hashes
    let c = OpStrategy::FloatInstr;
    // Not guaranteed but extremely likely for these simple variants
    assert_ne!(hash_of(&a), hash_of(&c));
}

// ReturnTag tests

#[test]
fn return_tag_from_type_tag_conversion() {
    let tag = ReturnTag::from(TypeTag::Int);
    assert_eq!(tag, ReturnTag::Concrete(TypeTag::Int));
}

#[test]
fn return_tag_is_const_constructible() {
    const SELF: ReturnTag = ReturnTag::SelfType;
    const UNIT: ReturnTag = ReturnTag::Unit;
    const ELEM: ReturnTag = ReturnTag::ElementType;
    const FRESH: ReturnTag = ReturnTag::Fresh;
    const LIST_STR: ReturnTag = ReturnTag::List(TypeTag::Str);
    const OPT_OF_ELEM: ReturnTag = ReturnTag::OptionOf(TypeProjection::Element);
    const NEXT: ReturnTag = ReturnTag::NextResult;

    assert_eq!(SELF, ReturnTag::SelfType);
    assert_eq!(UNIT, ReturnTag::Unit);
    assert_eq!(ELEM, ReturnTag::ElementType);
    assert_eq!(FRESH, ReturnTag::Fresh);
    assert_eq!(LIST_STR, ReturnTag::List(TypeTag::Str));
    assert_eq!(OPT_OF_ELEM, ReturnTag::OptionOf(TypeProjection::Element));
    assert_eq!(NEXT, ReturnTag::NextResult);
}

// TypeProjection tests

#[test]
fn type_projection_is_const_constructible() {
    const ELEM: TypeProjection = TypeProjection::Element;
    const KEY: TypeProjection = TypeProjection::Key;
    const VAL: TypeProjection = TypeProjection::Value;
    const OK: TypeProjection = TypeProjection::Ok;
    const ERR: TypeProjection = TypeProjection::Err;
    const FIXED: TypeProjection = TypeProjection::Fixed(TypeTag::Int);

    assert_eq!(ELEM, TypeProjection::Element);
    assert_eq!(KEY, TypeProjection::Key);
    assert_eq!(VAL, TypeProjection::Value);
    assert_eq!(OK, TypeProjection::Ok);
    assert_eq!(ERR, TypeProjection::Err);
    assert_eq!(FIXED, TypeProjection::Fixed(TypeTag::Int));
}

// TypeParamArity tests

#[test]
fn type_param_arity_fixed_values() {
    assert_eq!(TypeParamArity::Fixed(0), TypeParamArity::Fixed(0));
    assert_ne!(TypeParamArity::Fixed(0), TypeParamArity::Fixed(1));
    assert_ne!(TypeParamArity::Fixed(1), TypeParamArity::Fixed(2));
    assert_ne!(TypeParamArity::Fixed(0), TypeParamArity::Variadic);
}

#[test]
fn type_param_arity_is_const_constructible() {
    const ZERO: TypeParamArity = TypeParamArity::Fixed(0);
    const ONE: TypeParamArity = TypeParamArity::Fixed(1);
    const TWO: TypeParamArity = TypeParamArity::Fixed(2);
    const VAR: TypeParamArity = TypeParamArity::Variadic;

    assert_eq!(ZERO, TypeParamArity::Fixed(0));
    assert_eq!(ONE, TypeParamArity::Fixed(1));
    assert_eq!(TWO, TypeParamArity::Fixed(2));
    assert_eq!(VAR, TypeParamArity::Variadic);
}

// MethodKind tests

#[test]
fn method_kind_variants_distinct() {
    assert_ne!(MethodKind::Instance, MethodKind::Associated);
}

#[test]
fn method_kind_is_const_constructible() {
    const INST: MethodKind = MethodKind::Instance;
    const ASSOC: MethodKind = MethodKind::Associated;
    assert_eq!(INST, MethodKind::Instance);
    assert_eq!(ASSOC, MethodKind::Associated);
}

// DeiPropagation tests

#[test]
fn dei_propagation_all_three_distinct() {
    assert_ne!(DeiPropagation::Propagate, DeiPropagation::Downgrade);
    assert_ne!(DeiPropagation::Downgrade, DeiPropagation::NotApplicable);
    assert_ne!(DeiPropagation::NotApplicable, DeiPropagation::Propagate);
}

#[test]
fn dei_propagation_is_const_constructible() {
    const P: DeiPropagation = DeiPropagation::Propagate;
    const D: DeiPropagation = DeiPropagation::Downgrade;
    const N: DeiPropagation = DeiPropagation::NotApplicable;
    assert_eq!(P, DeiPropagation::Propagate);
    assert_eq!(D, DeiPropagation::Downgrade);
    assert_eq!(N, DeiPropagation::NotApplicable);
}
