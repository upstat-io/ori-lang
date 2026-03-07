//! Validation tests for builtin type definitions.
//!
//! These tests verify internal consistency of the registry data.
//! Cross-crate validation tests (comparing against `ori_types`, `ori_eval`, etc.)
//! are deferred to Section 09/14 where the dependency exists.

use crate::defs::*;
use crate::{MemoryStrategy, OpStrategy, TypeParamArity};

// Primitive type constants used across tests.
const PRIMITIVE_TYPES: &[&crate::TypeDef] = &[&INT, &FLOAT, &BOOL, &BYTE, &CHAR];

// 03.6a Registry-internal tests

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

// 03.6.1 Operator strategy correctness tests

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

// 03.6.2 OpDefs field coverage test

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

// 04.5 STR type definition tests

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
