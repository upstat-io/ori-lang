//! Tests for the Error `TypeDef`.

use super::*;

#[test]
fn error_method_count() {
    assert_eq!(ERROR.methods.len(), 8);
}

#[test]
fn error_is_arc() {
    assert_eq!(ERROR.memory, MemoryStrategy::Arc);
}

#[test]
fn error_all_methods_borrow_receiver() {
    for m in ERROR.methods {
        assert_eq!(
            m.receiver,
            Ownership::Borrow,
            "Error.{} should borrow receiver",
            m.name
        );
    }
}

#[test]
fn error_no_backend_required() {
    for m in ERROR.methods {
        assert!(
            !m.backend_required,
            "Error.{} should not be backend_required",
            m.name
        );
    }
}

#[test]
fn error_all_instance_methods() {
    for m in ERROR.methods {
        assert_eq!(
            m.kind,
            crate::MethodKind::Instance,
            "Error.{} should be Instance",
            m.name
        );
    }
}

#[test]
fn error_all_pure() {
    for m in ERROR.methods {
        assert!(m.pure, "Error.{} should be pure", m.name);
    }
}

#[test]
fn error_no_operators() {
    let ops = &ERROR.operators;
    assert_eq!(ops.add, OpStrategy::Unsupported);
    assert_eq!(ops.sub, OpStrategy::Unsupported);
    assert_eq!(ops.eq, OpStrategy::Unsupported);
    assert_eq!(ops.neq, OpStrategy::Unsupported);
    assert_eq!(ops.neg, OpStrategy::Unsupported);
}

#[test]
fn error_format_not_in_methods() {
    assert!(
        ERROR.methods.iter().all(|m| m.name != "format"),
        "format is blanket from Printable — should not be in ERROR_METHODS"
    );
}

#[test]
fn error_trait_methods_have_trait_name() {
    let expected = [
        ("clone", "Clone"),
        ("debug", "Debug"),
        ("has_trace", "Traceable"),
        ("to_str", "Printable"),
        ("trace", "Traceable"),
        ("trace_entries", "Traceable"),
        ("with_trace", "Traceable"),
    ];
    for (name, trait_name) in expected {
        let m = ERROR
            .methods
            .iter()
            .find(|m| m.name == name)
            .unwrap_or_else(|| panic!("trait method should exist"));
        assert_eq!(m.trait_name, Some(trait_name), "Error.{name}");
    }
}

#[test]
fn error_message_has_no_trait() {
    let m = ERROR
        .methods
        .iter()
        .find(|m| m.name == "message")
        .unwrap_or_else(|| panic!("message method should exist"));
    assert_eq!(m.trait_name, None, "Error.message should have no trait");
}

#[test]
fn error_no_dei() {
    for m in ERROR.methods {
        assert!(!m.dei_only, "Error.{} should not be dei_only", m.name);
    }
}
