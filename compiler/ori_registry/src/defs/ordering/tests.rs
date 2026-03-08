//! Tests for the Ordering `TypeDef`.

use super::*;

#[test]
fn ordering_method_count() {
    assert_eq!(ORDERING.methods.len(), 14);
}

#[test]
fn ordering_is_copy() {
    assert_eq!(ORDERING.memory, MemoryStrategy::Copy);
}

#[test]
fn ordering_predicate_methods_return_bool() {
    let predicates = [
        "is_less",
        "is_equal",
        "is_greater",
        "is_less_or_equal",
        "is_greater_or_equal",
    ];
    for name in predicates {
        let m = ORDERING
            .methods
            .iter()
            .find(|m| m.name == name)
            .unwrap_or_else(|| panic!("predicate method should exist"));
        assert_eq!(m.returns, BOOL, "Ordering.{name} should return bool");
    }
}

#[test]
fn ordering_all_methods_are_instance() {
    for m in ORDERING.methods {
        assert_eq!(
            m.kind,
            crate::MethodKind::Instance,
            "Ordering.{} should be Instance",
            m.name
        );
    }
}

#[test]
fn ordering_to_int_not_in_methods() {
    assert!(
        ORDERING.methods.iter().all(|m| m.name != "to_int"),
        "to_int is LLVM-only — should not be in ORDERING_METHODS"
    );
}

#[test]
fn ordering_format_not_in_methods() {
    assert!(
        ORDERING.methods.iter().all(|m| m.name != "format"),
        "format is blanket from Printable — should not be in ORDERING_METHODS"
    );
}

#[test]
fn ordering_no_comparison_operators() {
    assert_eq!(ORDERING.operators.lt, OpStrategy::Unsupported);
    assert_eq!(ORDERING.operators.gt, OpStrategy::Unsupported);
    assert_eq!(ORDERING.operators.lt_eq, OpStrategy::Unsupported);
    assert_eq!(ORDERING.operators.gt_eq, OpStrategy::Unsupported);
}

#[test]
fn ordering_eq_neq_use_int_instr() {
    assert_eq!(ORDERING.operators.eq, OpStrategy::IntInstr);
    assert_eq!(ORDERING.operators.neq, OpStrategy::IntInstr);
}

#[test]
fn ordering_variant_count() {
    assert_eq!(ORDERING_VARIANTS.len(), 3);
}

#[test]
fn ordering_variant_tags() {
    assert_eq!(ORDERING_VARIANTS[0].name, "Less");
    assert_eq!(ORDERING_VARIANTS[0].tag, 0);
    assert_eq!(ORDERING_VARIANTS[1].name, "Equal");
    assert_eq!(ORDERING_VARIANTS[1].tag, 1);
    assert_eq!(ORDERING_VARIANTS[2].name, "Greater");
    assert_eq!(ORDERING_VARIANTS[2].tag, 2);
}

#[test]
fn ordering_then_with_not_backend_required() {
    let m = ORDERING
        .methods
        .iter()
        .find(|m| m.name == "then_with")
        .unwrap_or_else(|| panic!("then_with method should exist"));
    assert!(!m.backend_required, "then_with is typeck-only");
}

#[test]
fn ordering_trait_methods_have_trait_name() {
    let expected = [
        ("equals", "Eq"),
        ("compare", "Comparable"),
        ("clone", "Clone"),
        ("hash", "Hashable"),
        ("to_str", "Printable"),
        ("debug", "Debug"),
    ];
    for (name, trait_name) in expected {
        let m = ORDERING
            .methods
            .iter()
            .find(|m| m.name == name)
            .unwrap_or_else(|| panic!("trait method should exist"));
        assert_eq!(m.trait_name, Some(trait_name), "Ordering.{name}");
    }
}
