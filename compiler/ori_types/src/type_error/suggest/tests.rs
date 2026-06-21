use super::*;

#[test]
fn int_float_suggestions() {
    let problem = TypeProblem::IntFloat {
        expected: "int",
        found: "float",
    };
    let suggestions = problem.suggestions();
    assert_eq!(suggestions.len(), 1);
    assert!(suggestions[0].message.contains("int(x)"));
}

#[test]
fn needs_unwrap_suggestions() {
    let problem = TypeProblem::NeedsUnwrap {
        inner_type: crate::Idx::INT,
    };
    let suggestions = problem.suggestions();
    assert!(!suggestions.is_empty());
    assert!(suggestions[0].message.contains('?'));
}

#[test]
fn wrong_arity_suggestions() {
    let problem = TypeProblem::WrongArity {
        expected: 2,
        found: 4,
    };
    let suggestions = problem.suggestions();
    assert_eq!(suggestions.len(), 1);
    assert!(suggestions[0].message.contains("remove"));
    assert!(suggestions[0].message.contains('2'));
}

#[test]
fn suggestion_priority_sorting() {
    let problem = TypeProblem::NeedsUnwrap {
        inner_type: crate::Idx::INT,
    };
    let suggestions = problem.suggestions();

    // Check that suggestions are sorted by priority
    for i in 1..suggestions.len() {
        assert!(suggestions[i - 1].priority <= suggestions[i].priority);
    }
}

// Regression: deferred-render suggestions share one `{N}` template across
// distinct `Name` operands; the dedup in `generate_suggestions` must key on
// (message, names) so two distinct missing-field suggestions are not collapsed.
#[test]
fn missing_field_suggestions_carry_distinct_names_for_dedup() {
    use ori_ir::StringInterner;
    let interner = StringInterner::new();
    let foo = interner.intern("foo");
    let bar = interner.intern("bar");

    let s_foo = TypeProblem::MissingField {
        field_name: foo,
        available: vec![],
    }
    .suggestions();
    let s_bar = TypeProblem::MissingField {
        field_name: bar,
        available: vec![],
    }
    .suggestions();

    // Same `{0}` template (so message-only dedup would collapse them)...
    assert_eq!(s_foo[0].message, s_bar[0].message);
    // ...but distinct `Name` operands keep them distinct under (message, names).
    assert_ne!(s_foo[0].names, s_bar[0].names);
}
