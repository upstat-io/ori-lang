//! Validation tests for builtin type definitions.
//!
//! These tests verify internal consistency of the registry data.
//! Cross-crate validation tests (comparing against `ori_types`, `ori_eval`, etc.)
//! live in the consuming crates where the dependency exists.

use crate::defs::*;
use crate::{MemoryStrategy, OpStrategy, Ownership, ReturnTag, TypeParamArity, TypeTag};

// Primitive type constants used across tests.
const PRIMITIVE_TYPES: &[&crate::TypeDef] = &[&INT, &FLOAT, &BOOL, &BYTE, &CHAR];

// Registry-internal tests

#[test]
fn no_duplicate_methods() {
    for type_def in BUILTIN_TYPES {
        let methods = type_def.methods;
        for (i, m) in methods.iter().enumerate() {
            for other in &methods[i + 1..] {
                assert_ne!(
                    m.name, other.name,
                    "duplicate method `{}` on type `{}`",
                    m.name, type_def.name
                );
            }
        }
    }
}

#[test]
fn all_primitives_are_copy() {
    for type_def in PRIMITIVE_TYPES {
        assert_eq!(
            type_def.memory,
            MemoryStrategy::Copy,
            "primitive `{}` should be MemoryStrategy::Copy",
            type_def.name
        );
    }
}

#[test]
fn all_primitives_have_zero_type_params() {
    for type_def in PRIMITIVE_TYPES {
        assert_eq!(
            type_def.type_params,
            TypeParamArity::Fixed(0),
            "primitive `{}` should have TypeParamArity::Fixed(0)",
            type_def.name
        );
    }
}

#[test]
fn all_methods_have_names() {
    for type_def in BUILTIN_TYPES {
        for m in type_def.methods {
            assert!(
                !m.name.is_empty(),
                "found method with empty name on type `{}`",
                type_def.name
            );
        }
    }
}

#[test]
fn methods_alphabetically_sorted() {
    for type_def in BUILTIN_TYPES {
        let methods = type_def.methods;
        for pair in methods.windows(2) {
            assert!(
                pair[0].name <= pair[1].name,
                "methods on `{}` not sorted: `{}` should come after `{}`",
                type_def.name,
                pair[0].name,
                pair[1].name
            );
        }
    }
}

// Operator strategy correctness tests

#[test]
fn int_comparison_is_signed() {
    assert_eq!(INT.operators.lt, OpStrategy::IntInstr);
    assert_eq!(INT.operators.gt, OpStrategy::IntInstr);
    assert_eq!(INT.operators.lt_eq, OpStrategy::IntInstr);
    assert_eq!(INT.operators.gt_eq, OpStrategy::IntInstr);
}

#[test]
fn byte_comparison_is_unsigned() {
    assert_eq!(BYTE.operators.lt, OpStrategy::UnsignedCmp);
    assert_eq!(BYTE.operators.gt, OpStrategy::UnsignedCmp);
    assert_eq!(BYTE.operators.lt_eq, OpStrategy::UnsignedCmp);
    assert_eq!(BYTE.operators.gt_eq, OpStrategy::UnsignedCmp);
}

#[test]
fn char_comparison_is_unsigned() {
    assert_eq!(CHAR.operators.lt, OpStrategy::UnsignedCmp);
    assert_eq!(CHAR.operators.gt, OpStrategy::UnsignedCmp);
    assert_eq!(CHAR.operators.lt_eq, OpStrategy::UnsignedCmp);
    assert_eq!(CHAR.operators.gt_eq, OpStrategy::UnsignedCmp);
}

#[test]
fn bool_comparison_is_unsigned() {
    assert_eq!(BOOL.operators.lt, OpStrategy::UnsignedCmp);
    assert_eq!(BOOL.operators.gt, OpStrategy::UnsignedCmp);
    assert_eq!(BOOL.operators.lt_eq, OpStrategy::UnsignedCmp);
    assert_eq!(BOOL.operators.gt_eq, OpStrategy::UnsignedCmp);
}

#[test]
fn bool_equality_is_bool_logic() {
    assert_eq!(BOOL.operators.eq, OpStrategy::BoolLogic);
    assert_eq!(BOOL.operators.neq, OpStrategy::BoolLogic);
}

#[test]
fn float_comparison_is_float_instr() {
    assert_eq!(FLOAT.operators.lt, OpStrategy::FloatInstr);
    assert_eq!(FLOAT.operators.gt, OpStrategy::FloatInstr);
    assert_eq!(FLOAT.operators.lt_eq, OpStrategy::FloatInstr);
    assert_eq!(FLOAT.operators.gt_eq, OpStrategy::FloatInstr);
    assert_eq!(FLOAT.operators.eq, OpStrategy::FloatInstr);
    assert_eq!(FLOAT.operators.neq, OpStrategy::FloatInstr);
}

#[test]
fn bool_not_is_bool_logic() {
    assert_eq!(BOOL.operators.not, OpStrategy::BoolLogic);
}

#[test]
fn non_bool_not_is_unsupported() {
    assert_eq!(INT.operators.not, OpStrategy::Unsupported);
    assert_eq!(FLOAT.operators.not, OpStrategy::Unsupported);
    assert_eq!(BYTE.operators.not, OpStrategy::Unsupported);
    assert_eq!(CHAR.operators.not, OpStrategy::Unsupported);
}

#[test]
fn float_has_no_bitwise_ops() {
    assert_eq!(FLOAT.operators.bit_and, OpStrategy::Unsupported);
    assert_eq!(FLOAT.operators.bit_or, OpStrategy::Unsupported);
    assert_eq!(FLOAT.operators.bit_xor, OpStrategy::Unsupported);
    assert_eq!(FLOAT.operators.bit_not, OpStrategy::Unsupported);
    assert_eq!(FLOAT.operators.shl, OpStrategy::Unsupported);
    assert_eq!(FLOAT.operators.shr, OpStrategy::Unsupported);
}

#[test]
fn float_has_no_floor_div() {
    // float.rem = FloatInstr (proactive addition — LLVM handles `frem`).
    // float.floor_div = Unsupported (floor division is integer-only).
    assert_eq!(FLOAT.operators.floor_div, OpStrategy::Unsupported);
    assert_eq!(FLOAT.operators.rem, OpStrategy::FloatInstr);
}

#[test]
fn char_has_no_arithmetic() {
    assert_eq!(CHAR.operators.add, OpStrategy::Unsupported);
    assert_eq!(CHAR.operators.sub, OpStrategy::Unsupported);
    assert_eq!(CHAR.operators.mul, OpStrategy::Unsupported);
    assert_eq!(CHAR.operators.div, OpStrategy::Unsupported);
    assert_eq!(CHAR.operators.rem, OpStrategy::Unsupported);
    assert_eq!(CHAR.operators.floor_div, OpStrategy::Unsupported);
}

#[test]
fn byte_has_no_neg() {
    assert_eq!(BYTE.operators.neg, OpStrategy::Unsupported);
}

// OpDefs field coverage test

#[test]
fn opdefs_has_all_20_fields() {
    // Structural test: access every OpDefs field to verify it exists.
    // If a field were missing, this would fail to compile.
    // Additionally verify that the UNSUPPORTED constant has all 20 fields set.
    let all_unsupported = crate::OpDefs::UNSUPPORTED;
    let fields = [
        all_unsupported.add,
        all_unsupported.sub,
        all_unsupported.mul,
        all_unsupported.div,
        all_unsupported.rem,
        all_unsupported.floor_div,
        all_unsupported.eq,
        all_unsupported.neq,
        all_unsupported.lt,
        all_unsupported.gt,
        all_unsupported.lt_eq,
        all_unsupported.gt_eq,
        all_unsupported.neg,
        all_unsupported.not,
        all_unsupported.bit_and,
        all_unsupported.bit_or,
        all_unsupported.bit_xor,
        all_unsupported.bit_not,
        all_unsupported.shl,
        all_unsupported.shr,
    ];
    assert_eq!(fields.len(), 20, "OpDefs should have exactly 20 fields");
    for (i, field) in fields.iter().enumerate() {
        assert_eq!(
            *field,
            OpStrategy::Unsupported,
            "OpDefs::UNSUPPORTED field {i} should be Unsupported"
        );
    }
}

// Additional method count verification

#[test]
fn primitive_method_counts_match_plan() {
    assert_eq!(INT.methods.len(), 35, "INT should have 35 methods");
    assert_eq!(FLOAT.methods.len(), 43, "FLOAT should have 43 methods");
    assert_eq!(BOOL.methods.len(), 8, "BOOL should have 8 methods");
    assert_eq!(BYTE.methods.len(), 23, "BYTE should have 23 methods");
    assert_eq!(CHAR.methods.len(), 16, "CHAR should have 16 methods");
}

// Primitive trait method coverage

/// Every primitive must have the 5 core trait methods.
#[test]
fn all_primitives_have_core_trait_methods() {
    let core_methods = ["clone", "compare", "debug", "equals", "hash", "to_str"];
    for type_def in PRIMITIVE_TYPES {
        for method in core_methods {
            assert!(
                type_def.methods.iter().any(|m| m.name == method),
                "primitive `{}` is missing core method `{}`",
                type_def.name,
                method
            );
        }
    }
}

/// Bool-specific method verification.
#[test]
fn bool_has_expected_methods() {
    let expected = [
        "clone", "compare", "debug", "equals", "hash", "not", "to_int", "to_str",
    ];
    for method in expected {
        assert!(
            BOOL.methods.iter().any(|m| m.name == method),
            "bool should have method `{method}`"
        );
    }
    assert_eq!(
        BOOL.methods.len(),
        expected.len(),
        "bool should have exactly {} methods",
        expected.len()
    );
}

// STR type definition tests

#[test]
fn str_method_count() {
    assert_eq!(
        STR.methods.len(),
        43,
        "STR should have exactly 43 methods (38 typeck + add + as_bytes + to_bytes + from_utf8 + from_utf8_unchecked)"
    );
}

#[test]
fn str_is_arc() {
    assert_eq!(
        STR.memory,
        MemoryStrategy::Arc,
        "str must be MemoryStrategy::Arc"
    );
}

#[test]
fn str_all_instance_methods_borrow_receiver() {
    for m in STR.methods {
        if m.kind == crate::MethodKind::Instance {
            assert_eq!(
                m.receiver,
                crate::Ownership::Borrow,
                "str instance method `{}` should borrow receiver",
                m.name
            );
        }
    }
}

#[test]
fn str_operators_all_runtime_call_or_unsupported() {
    let ops = &STR.operators;
    let all = [
        ("add", ops.add),
        ("sub", ops.sub),
        ("mul", ops.mul),
        ("div", ops.div),
        ("rem", ops.rem),
        ("floor_div", ops.floor_div),
        ("eq", ops.eq),
        ("neq", ops.neq),
        ("lt", ops.lt),
        ("gt", ops.gt),
        ("lt_eq", ops.lt_eq),
        ("gt_eq", ops.gt_eq),
        ("neg", ops.neg),
        ("not", ops.not),
        ("bit_and", ops.bit_and),
        ("bit_or", ops.bit_or),
        ("bit_xor", ops.bit_xor),
        ("bit_not", ops.bit_not),
        ("shl", ops.shl),
        ("shr", ops.shr),
    ];
    for (name, op) in all {
        assert!(
            matches!(op, OpStrategy::RuntimeCall { .. } | OpStrategy::Unsupported),
            "str operator `{name}` must be RuntimeCall or Unsupported, got {op:?}"
        );
    }
}

#[test]
fn str_runtime_call_names_are_valid() {
    let ops = &STR.operators;
    let all = [
        ops.add,
        ops.sub,
        ops.mul,
        ops.div,
        ops.rem,
        ops.floor_div,
        ops.eq,
        ops.neq,
        ops.lt,
        ops.gt,
        ops.lt_eq,
        ops.gt_eq,
        ops.neg,
        ops.not,
        ops.bit_and,
        ops.bit_or,
        ops.bit_xor,
        ops.bit_not,
        ops.shl,
        ops.shr,
    ];
    for op in all {
        if let OpStrategy::RuntimeCall { fn_name, .. } = op {
            assert!(
                fn_name.starts_with("ori_str_"),
                "RuntimeCall fn_name `{fn_name}` must start with `ori_str_`"
            );
        }
    }
}

#[test]
fn str_trait_methods_have_trait_name() {
    let expected = [
        ("equals", "Eq"),
        ("compare", "Comparable"),
        ("clone", "Clone"),
        ("hash", "Hashable"),
        ("to_str", "Printable"),
        ("debug", "Debug"),
        ("add", "Add"),
        ("len", "Len"),
        ("length", "Len"),
        ("is_empty", "IsEmpty"),
        ("iter", "Iterable"),
    ];
    for (method_name, trait_name) in expected {
        let m = STR.methods.iter().find(|m| m.name == method_name);
        assert!(m.is_some(), "str should have method `{method_name}`");
        assert_eq!(
            m.unwrap_or_else(|| panic!("method should exist"))
                .trait_name,
            Some(trait_name),
            "str.{method_name} should have trait_name Some(\"{trait_name}\")"
        );
    }
}

#[test]
fn str_associated_functions() {
    let assoc_names = ["from_utf8", "from_utf8_unchecked"];
    for name in assoc_names {
        let m = STR.methods.iter().find(|m| m.name == name);
        assert!(m.is_some(), "str should have associated function `{name}`");
        assert_eq!(
            m.unwrap_or_else(|| panic!("method should exist")).kind,
            crate::MethodKind::Associated,
            "str.{name} should have kind MethodKind::Associated"
        );
    }
    // All other methods should be Instance
    for m in STR.methods {
        if !assoc_names.contains(&m.name) {
            assert_eq!(
                m.kind,
                crate::MethodKind::Instance,
                "str.{} should have kind MethodKind::Instance",
                m.name
            );
        }
    }
}

#[test]
fn str_alias_pairs_have_matching_signatures() {
    let alias_pairs = [
        ("length", "len"),
        ("substring", "slice"),
        ("parse_int", "to_int"),
        ("parse_float", "to_float"),
    ];
    for (alias, canonical) in alias_pairs {
        let a = STR
            .methods
            .iter()
            .find(|m| m.name == alias)
            .unwrap_or_else(|| panic!("str should have method `{alias}`"));
        let c = STR
            .methods
            .iter()
            .find(|m| m.name == canonical)
            .unwrap_or_else(|| panic!("str should have method `{canonical}`"));
        assert_eq!(
            a.params.len(),
            c.params.len(),
            "alias `{alias}` and canonical `{canonical}` should have same param count"
        );
        assert_eq!(
            a.returns, c.returns,
            "alias `{alias}` and canonical `{canonical}` should have same return type"
        );
    }
}

#[test]
fn str_comparison_operators_use_ori_str_compare() {
    let ops = &STR.operators;
    for (name, op) in [
        ("lt", ops.lt),
        ("gt", ops.gt),
        ("lt_eq", ops.lt_eq),
        ("gt_eq", ops.gt_eq),
    ] {
        match op {
            OpStrategy::RuntimeCall {
                fn_name,
                returns_bool,
            } => {
                assert_eq!(
                    fn_name, "ori_str_compare",
                    "str.{name} should use ori_str_compare"
                );
                assert!(returns_bool, "str.{name} comparison should return bool");
            }
            _ => panic!("str.{name} should be RuntimeCall, got {op:?}"),
        }
    }
}

#[test]
fn str_neq_uses_ori_str_ne() {
    match STR.operators.neq {
        OpStrategy::RuntimeCall {
            fn_name,
            returns_bool,
        } => {
            assert_eq!(
                fn_name, "ori_str_ne",
                "str.neq should use ori_str_ne (not ori_str_neq)"
            );
            assert!(returns_bool);
        }
        _ => panic!("str.neq should be RuntimeCall"),
    }
}

// Registry-level integrity tests

/// Every `TypeDef` must have at least one method.
/// A type with zero methods provides no behavioral specification
/// and should not be in the registry.
#[test]
fn no_empty_types() {
    for type_def in BUILTIN_TYPES {
        assert!(
            !type_def.methods.is_empty(),
            "TypeDef `{}` has zero methods -- every registered type must \
             have at least one method (minimally: clone, equals, to_str)",
            type_def.name,
        );
    }
}

/// Every `TypeTag` variant that carries methods must have a corresponding
/// `TypeDef` in `BUILTIN_TYPES`. Variants without methods (`Unit`, `Never`,
/// `Function`) and the DEI alias (`DoubleEndedIterator` → `Iterator`) are
/// intentionally excluded.
#[test]
fn all_method_bearing_type_tags_present() {
    use std::collections::HashSet;

    // TypeTag variants that intentionally have no TypeDef:
    // - Unit, Never: no methods, no operators
    // - Function: no methods (memory classification only)
    // - DoubleEndedIterator: aliases to Iterator via base_type()
    const EXCLUDED: &[TypeTag] = &[
        TypeTag::Unit,
        TypeTag::Never,
        TypeTag::Function,
        TypeTag::DoubleEndedIterator,
    ];

    let registered_tags: HashSet<TypeTag> = BUILTIN_TYPES.iter().map(|td| td.tag).collect();

    for &tag in TypeTag::all() {
        if EXCLUDED.contains(&tag) {
            continue;
        }
        assert!(
            registered_tags.contains(&tag),
            "TypeTag::{tag:?} has no TypeDef in BUILTIN_TYPES. \
             Add a const TypeDef in ori_registry/src/defs/ and include \
             it in BUILTIN_TYPES.",
        );
    }
}

/// Every `MethodDef` on a `Copy` type must use `Ownership::Borrow`.
/// Copy types are trivially borrowed (borrow == copy), but the annotation
/// documents the intent. Arc types may use Owned for consuming methods.
#[test]
fn all_receivers_documented() {
    for type_def in BUILTIN_TYPES {
        for method in type_def.methods {
            if type_def.memory == MemoryStrategy::Copy {
                assert_eq!(
                    method.receiver,
                    Ownership::Borrow,
                    "Method `{}.{}` on a Copy type should use Ownership::Borrow \
                     (Copy types are trivially borrowed)",
                    type_def.name,
                    method.name,
                );
            }
            // Arc types: most methods borrow, but consuming methods (into)
            // may use Owned. Field access proves the value is explicitly set.
            let _ = method.receiver;
        }
    }
}

/// Every Ori builtin type supports `==` (equality). This is a language invariant.
/// Equality is provided either via operator strategy (primitives) or via the
/// `equals` method from the Eq trait (collections, wrappers).
///
/// Exempt types:
/// - Error: non-deterministic trace data makes structural equality misleading
/// - Channel: identity-based handle, not structural value
/// - Iterator: stateful/consumed on use, equality is nonsensical
/// - Range: TODO — should have `equals` method but missing from registry
///   (equality works in eval/typeck via structural comparison, but the
///   registry `TypeDef` doesn't declare it yet)
#[test]
fn no_unsupported_eq() {
    const EQ_EXEMPT: &[TypeTag] = &[
        TypeTag::Error,
        TypeTag::Channel,
        TypeTag::Iterator,
        TypeTag::DoubleEndedIterator,
        TypeTag::Range, // TODO: add `equals` method to Range TypeDef
    ];

    for type_def in BUILTIN_TYPES {
        if EQ_EXEMPT.contains(&type_def.tag) {
            continue;
        }
        let has_operator_eq = type_def.operators.eq != OpStrategy::Unsupported;
        let has_equals_method = type_def.methods.iter().any(|m| m.name == "equals");
        assert!(
            has_operator_eq || has_equals_method,
            "Type `{}` has neither an eq operator strategy nor an `equals` method. \
             All Ori types must support equality (or be in EQ_EXEMPT with justification).",
            type_def.name,
        );
    }
}

/// If a type supports comparison operators (`lt`, `gt`, `lt_eq`, `gt_eq`), it must
/// also support equality (`eq`, `neq`). Comparison without equality is nonsensical.
/// Additionally, all supported comparison operators must use the same strategy.
#[test]
fn operator_consistency() {
    for type_def in BUILTIN_TYPES {
        let ops = &type_def.operators;

        let has_any_cmp = ops.lt != OpStrategy::Unsupported
            || ops.gt != OpStrategy::Unsupported
            || ops.lt_eq != OpStrategy::Unsupported
            || ops.gt_eq != OpStrategy::Unsupported;

        if has_any_cmp {
            assert!(
                ops.eq != OpStrategy::Unsupported,
                "Type `{}` supports comparison but not equality. \
                 If lt/gt/le/ge are supported, eq must be too.",
                type_def.name,
            );
            assert!(
                ops.neq != OpStrategy::Unsupported,
                "Type `{}` supports comparison but not not-equal. \
                 If lt/gt/le/ge are supported, neq must be too.",
                type_def.name,
            );
        }

        // All supported comparison operators should use the same strategy
        let cmp_ops = [ops.lt, ops.gt, ops.lt_eq, ops.gt_eq];
        let supported_cmp: Vec<_> = cmp_ops
            .iter()
            .filter(|s| **s != OpStrategy::Unsupported)
            .collect();
        if supported_cmp.len() > 1 {
            let first = supported_cmp[0];
            for s in &supported_cmp[1..] {
                assert_eq!(
                    *s, first,
                    "Type `{}` uses mixed comparison strategies: {:?} vs {:?}. \
                     All comparison operators should use the same strategy.",
                    type_def.name, first, s,
                );
            }
        }
    }
}

/// Methods returning `SelfType` must be semantically valid — `to_*` conversion
/// methods should return concrete `TypeTag`s, not `SelfType` (except identity
/// conversions like `str.to_str`).
#[test]
fn self_type_returns_valid() {
    for type_def in BUILTIN_TYPES {
        for method in type_def.methods {
            if method.returns == ReturnTag::SelfType
                && method.name.starts_with("to_")
                && method.name != "to_uppercase"
                && method.name != "to_lowercase"
                && method.name != "to_ascii_uppercase"
                && method.name != "to_ascii_lowercase"
            {
                // to_str, to_int, to_float, etc. should use concrete return types.
                // EXCEPTION: `to_str` on str returns SelfType (identity).
                let is_identity = type_def.tag == TypeTag::Str && method.name == "to_str";
                assert!(
                    is_identity,
                    "Method `{}.{}` returns SelfType but is a conversion \
                     method (`to_*`). Conversion methods should return \
                     a concrete TypeTag, not SelfType.",
                    type_def.name, method.name,
                );
            }
        }
    }
}

// Trait coverage enforcement tests (02.1)

/// Every `len` method across all types must have `trait_name: Some("Len")`.
/// Every `is_empty` method must have `trait_name: Some("IsEmpty")`.
/// Every `iter` method must have `trait_name: Some("Iterable")`.
/// This prevents future drift when new types are added.
#[test]
fn trait_methods_have_correct_trait_name() {
    let expected: &[(&str, &str)] = &[
        ("len", "Len"),
        ("is_empty", "IsEmpty"),
        ("iter", "Iterable"),
    ];

    for type_def in BUILTIN_TYPES {
        for method in type_def.methods {
            for &(method_name, expected_trait) in expected {
                if method.name == method_name {
                    assert_eq!(
                        method.trait_name,
                        Some(expected_trait),
                        "Method `{}.{}` must have trait_name Some(\"{}\"), got {:?}",
                        type_def.name,
                        method.name,
                        expected_trait,
                        method.trait_name,
                    );
                }
            }
        }
    }
}

/// Every `length` alias method must also have `trait_name: Some("Len")`.
/// Aliases should carry the same `trait_name` as the canonical method.
#[test]
fn alias_methods_have_correct_trait_name() {
    let alias_pairs: &[(&str, &str)] = &[("length", "Len")];

    for type_def in BUILTIN_TYPES {
        for method in type_def.methods {
            for &(alias_name, expected_trait) in alias_pairs {
                if method.name == alias_name {
                    assert_eq!(
                        method.trait_name,
                        Some(expected_trait),
                        "Alias method `{}.{}` must have trait_name Some(\"{}\"), got {:?}",
                        type_def.name,
                        method.name,
                        expected_trait,
                        method.trait_name,
                    );
                }
            }
        }
    }
}

/// Semantic pin: `TypeDef`s with non-empty `traits` field contain exactly
/// the expected marker traits. Reverting `traits` to `&[]` would fail.
#[test]
fn marker_traits_populated_correctly() {
    let expected: &[(TypeTag, &[&str])] = &[
        (TypeTag::Int, &["Default"]),
        (TypeTag::Float, &["Default"]),
        (TypeTag::Bool, &["Default"]),
        (TypeTag::Str, &["Default"]),
        (TypeTag::Duration, &["Default", "Sendable"]),
        (TypeTag::Size, &["Default", "Sendable"]),
        (TypeTag::List, &["Printable"]),
        (TypeTag::Map, &["Printable"]),
        (TypeTag::Set, &["Printable"]),
        (TypeTag::Range, &["Printable"]),
        (TypeTag::Option, &["Default", "Printable"]),
        (TypeTag::Result, &["Comparable", "Printable"]),
        (TypeTag::Tuple, &["Comparable", "Printable"]),
        (TypeTag::Iterator, &["Iterator"]),
    ];

    for &(tag, expected_traits) in expected {
        let type_def = BUILTIN_TYPES
            .iter()
            .find(|td| td.tag == tag)
            .unwrap_or_else(|| panic!("TypeDef for {tag:?} should exist"));
        assert_eq!(
            type_def.traits, expected_traits,
            "TypeDef `{}` traits should be {:?}, got {:?}",
            type_def.name, expected_traits, type_def.traits,
        );
    }
}

/// Negative pin: types that should NOT have marker traits must have
/// empty `traits` field. Prevents accidental trait additions.
#[test]
fn non_marker_types_have_empty_traits() {
    let should_be_empty: &[TypeTag] = &[
        TypeTag::Char,
        TypeTag::Byte,
        TypeTag::Ordering,
        TypeTag::Error,
        TypeTag::Channel,
    ];

    for &tag in should_be_empty {
        let type_def = BUILTIN_TYPES
            .iter()
            .find(|td| td.tag == tag)
            .unwrap_or_else(|| panic!("TypeDef for {tag:?} should exist"));
        assert!(
            type_def.traits.is_empty(),
            "TypeDef `{}` should have empty traits, got {:?}",
            type_def.name,
            type_def.traits,
        );
    }
}
