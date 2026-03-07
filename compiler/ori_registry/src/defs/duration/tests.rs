//! Tests for the Duration `TypeDef`.

use super::*;

#[test]
fn duration_method_count() {
    assert_eq!(DURATION.methods.len(), 41);
}

#[test]
fn duration_is_copy() {
    assert_eq!(DURATION.memory, MemoryStrategy::Copy);
}

#[test]
fn duration_associated_functions() {
    let assoc = [
        "from_hours",
        "from_micros",
        "from_microseconds",
        "from_millis",
        "from_milliseconds",
        "from_minutes",
        "from_nanos",
        "from_nanoseconds",
        "from_seconds",
        "zero",
    ];
    for name in assoc {
        let m = DURATION
            .methods
            .iter()
            .find(|m| m.name == name)
            .unwrap_or_else(|| panic!("associated function should exist"));
        assert_eq!(m.kind, crate::MethodKind::Associated, "Duration.{name}");
        assert!(
            !m.backend_required,
            "Duration.{name} should not be backend_required"
        );
    }
}

#[test]
fn duration_trait_methods() {
    let expected = [
        ("add", "Add"),
        ("sub", "Sub"),
        ("mul", "Mul"),
        ("div", "Div"),
        ("rem", "Rem"),
        ("neg", "Neg"),
        ("equals", "Eq"),
        ("compare", "Comparable"),
        ("clone", "Clone"),
        ("hash", "Hashable"),
        ("to_str", "Printable"),
        ("debug", "Debug"),
        ("format", "Formattable"),
    ];
    for (name, trait_name) in expected {
        let m = DURATION
            .methods
            .iter()
            .find(|m| m.name == name)
            .unwrap_or_else(|| panic!("trait method should exist"));
        assert_eq!(m.trait_name, Some(trait_name), "Duration.{name}");
    }
}

#[test]
fn duration_mul_div_take_int_not_self() {
    for name in ["mul", "div"] {
        let m = DURATION
            .methods
            .iter()
            .find(|m| m.name == name)
            .unwrap_or_else(|| panic!("method should exist"));
        assert_eq!(m.params.len(), 1);
        assert_eq!(
            m.params[0].ty,
            ReturnTag::Concrete(TypeTag::Int),
            "Duration.{name} should take int, not Self"
        );
    }
}

#[test]
fn duration_no_dei() {
    for m in DURATION.methods {
        assert!(!m.dei_only, "Duration.{} should not be dei_only", m.name);
    }
}

#[test]
fn duration_conversion_alias_pairs() {
    let pairs = [
        ("as_seconds", "to_seconds"),
        ("as_millis", "to_millis"),
        ("as_micros", "to_micros"),
        ("as_nanos", "to_nanos"),
    ];
    for (a, b) in pairs {
        let ma = DURATION
            .methods
            .iter()
            .find(|m| m.name == a)
            .unwrap_or_else(|| panic!("alias method should exist"));
        let mb = DURATION
            .methods
            .iter()
            .find(|m| m.name == b)
            .unwrap_or_else(|| panic!("canonical method should exist"));
        assert_eq!(
            ma.returns, mb.returns,
            "{a} and {b} should have same return type"
        );
        assert_eq!(
            ma.params.len(),
            mb.params.len(),
            "{a} and {b} should have same param count"
        );
    }
}

#[test]
fn duration_ops_int_instr_for_arithmetic() {
    let ops = &DURATION.operators;
    assert_eq!(ops.add, OpStrategy::IntInstr);
    assert_eq!(ops.sub, OpStrategy::IntInstr);
    assert_eq!(ops.mul, OpStrategy::IntInstr);
    assert_eq!(ops.div, OpStrategy::IntInstr);
    assert_eq!(ops.rem, OpStrategy::IntInstr);
    assert_eq!(ops.neg, OpStrategy::IntInstr);
}
