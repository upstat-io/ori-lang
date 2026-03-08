//! Tests for the Size `TypeDef`.

use super::*;

#[test]
fn size_method_count() {
    assert_eq!(SIZE.methods.len(), 34);
}

#[test]
fn size_is_copy() {
    assert_eq!(SIZE.memory, MemoryStrategy::Copy);
}

#[test]
fn size_associated_functions() {
    let assoc = [
        "from_bytes",
        "from_gb",
        "from_gigabytes",
        "from_kb",
        "from_kilobytes",
        "from_mb",
        "from_megabytes",
        "from_tb",
        "from_terabytes",
        "zero",
    ];
    for name in assoc {
        let m = SIZE
            .methods
            .iter()
            .find(|m| m.name == name)
            .unwrap_or_else(|| panic!("associated function should exist"));
        assert_eq!(m.kind, crate::MethodKind::Associated, "Size.{name}");
        assert!(
            !m.backend_required,
            "Size.{name} should not be backend_required"
        );
    }
}

#[test]
fn size_trait_methods() {
    let expected = [
        ("add", "Add"),
        ("sub", "Sub"),
        ("mul", "Mul"),
        ("div", "Div"),
        ("rem", "Rem"),
        ("equals", "Eq"),
        ("compare", "Comparable"),
        ("clone", "Clone"),
        ("hash", "Hashable"),
        ("to_str", "Printable"),
        ("debug", "Debug"),
        ("format", "Formattable"),
    ];
    for (name, trait_name) in expected {
        let m = SIZE
            .methods
            .iter()
            .find(|m| m.name == name)
            .unwrap_or_else(|| panic!("trait method should exist"));
        assert_eq!(m.trait_name, Some(trait_name), "Size.{name}");
    }
}

#[test]
fn size_mul_div_take_int_not_self() {
    for name in ["mul", "div"] {
        let m = SIZE
            .methods
            .iter()
            .find(|m| m.name == name)
            .unwrap_or_else(|| panic!("method should exist"));
        assert_eq!(m.params.len(), 1);
        assert_eq!(
            m.params[0].ty,
            ReturnTag::Concrete(TypeTag::Int),
            "Size.{name} should take int, not Self"
        );
    }
}

#[test]
fn size_no_neg_operator() {
    assert_eq!(SIZE.operators.neg, OpStrategy::Unsupported);
    assert!(
        SIZE.methods.iter().all(|m| m.name != "neg"),
        "Size should not have a neg method"
    );
}

#[test]
fn size_no_dei() {
    for m in SIZE.methods {
        assert!(!m.dei_only, "Size.{} should not be dei_only", m.name);
    }
}

#[test]
fn size_canonical_accessors_return_int() {
    let accessors = ["bytes", "kilobytes", "megabytes", "gigabytes", "terabytes"];
    for name in accessors {
        let m = SIZE
            .methods
            .iter()
            .find(|m| m.name == name)
            .unwrap_or_else(|| panic!("accessor method should exist"));
        assert_eq!(m.returns, INT, "Size.{name} should return int");
        assert!(m.backend_required, "Size.{name} should be backend_required");
    }
}

#[test]
fn size_alias_accessors_return_int() {
    let aliases = ["to_bytes", "as_bytes", "to_kb", "to_mb", "to_gb", "to_tb"];
    for name in aliases {
        let m = SIZE
            .methods
            .iter()
            .find(|m| m.name == name)
            .unwrap_or_else(|| panic!("alias accessor should exist"));
        assert_eq!(m.returns, INT, "Size.{name} should return int");
        assert!(
            !m.backend_required,
            "Size.{name} should not be backend_required"
        );
    }
}

#[test]
fn size_ops_int_instr_for_arithmetic() {
    let ops = &SIZE.operators;
    assert_eq!(ops.add, OpStrategy::IntInstr);
    assert_eq!(ops.sub, OpStrategy::IntInstr);
    assert_eq!(ops.mul, OpStrategy::IntInstr);
    assert_eq!(ops.div, OpStrategy::IntInstr);
    assert_eq!(ops.rem, OpStrategy::IntInstr);
}
