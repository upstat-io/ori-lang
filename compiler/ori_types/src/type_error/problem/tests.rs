use super::*;

#[test]
fn problem_severity() {
    assert_eq!(
        TypeProblem::IntFloat {
            expected: "int",
            found: "float"
        }
        .severity(),
        Severity::Error
    );
    assert_eq!(
        TypeProblem::FieldTypo {
            attempted: Name::from_raw(1),
            suggestion: Name::from_raw(2),
            distance: 1
        }
        .severity(),
        Severity::Warning
    );
}

#[test]
fn problem_descriptions() {
    assert_eq!(
        TypeProblem::IntFloat {
            expected: "int",
            found: "float"
        }
        .description(),
        "int and float are different types"
    );
    assert_eq!(
        TypeProblem::WrongArity {
            expected: 2,
            found: 3
        }
        .description(),
        "wrong number of arguments"
    );
}

#[test]
fn problem_hints() {
    assert!(TypeProblem::IntFloat {
        expected: "int",
        found: "float"
    }
    .hint()
    .is_some());
    assert!(TypeProblem::NeedsUnwrap {
        inner_type: Idx::INT
    }
    .hint()
    .is_some());
}

#[test]
fn problem_categories() {
    assert!(TypeProblem::IntFloat {
        expected: "int",
        found: "float"
    }
    .is_numeric());
    assert!(!TypeProblem::IntFloat {
        expected: "int",
        found: "float"
    }
    .is_function_related());

    assert!(TypeProblem::WrongArity {
        expected: 1,
        found: 2
    }
    .is_function_related());
    assert!(!TypeProblem::WrongArity {
        expected: 1,
        found: 2
    }
    .is_numeric());

    assert!(TypeProblem::MissingField {
        field_name: Name::from_raw(1),
        available: vec![]
    }
    .is_record_related());
}
