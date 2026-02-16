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

#[test]
fn top_suggestion() {
    let problem = TypeProblem::IntFloat {
        expected: "float",
        found: "int",
    };
    let top = problem.top_suggestion();
    assert!(top.is_some_and(|s| s.contains("float(x)")));
}
