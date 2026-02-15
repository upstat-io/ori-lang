use super::*;

#[test]
fn test_unknown_identifier() {
    let problem = SemanticProblem::UnknownIdentifier {
        span: Span::new(20, 25),
        name: Name::from_raw(1),
        similar: Some(Name::from_raw(2)),
    };

    assert_eq!(problem.span(), Span::new(20, 25));
    assert!(!problem.is_warning());
}

#[test]
fn test_duplicate_definition() {
    let problem = SemanticProblem::DuplicateDefinition {
        span: Span::new(100, 110),
        name: Name::from_raw(1),
        kind: DefinitionKind::Function,
        first_span: Span::new(10, 20),
    };

    assert_eq!(problem.span(), Span::new(100, 110));
    assert!(!problem.is_warning());
}

#[test]
fn test_unused_variable() {
    let problem = SemanticProblem::UnusedVariable {
        span: Span::new(5, 10),
        name: Name::from_raw(1),
    };

    assert_eq!(problem.span(), Span::new(5, 10));
    assert!(problem.is_warning());
}

#[test]
fn test_non_exhaustive_match() {
    let problem = SemanticProblem::NonExhaustiveMatch {
        span: Span::new(0, 50),
        missing_patterns: vec!["None".into(), "Some(Err(_))".into()],
    };

    assert_eq!(problem.span(), Span::new(0, 50));
    assert!(!problem.is_warning());
}

#[test]
fn test_definition_kind_display() {
    assert_eq!(DefinitionKind::Function.to_string(), "function");
    assert_eq!(DefinitionKind::Variable.to_string(), "variable");
    assert_eq!(DefinitionKind::Config.to_string(), "config");
    assert_eq!(DefinitionKind::Type.to_string(), "type");
}

#[test]
fn test_problem_equality() {
    let p1 = SemanticProblem::UnknownIdentifier {
        span: Span::new(20, 25),
        name: Name::from_raw(1),
        similar: Some(Name::from_raw(2)),
    };

    let p2 = SemanticProblem::UnknownIdentifier {
        span: Span::new(20, 25),
        name: Name::from_raw(1),
        similar: Some(Name::from_raw(2)),
    };

    let p3 = SemanticProblem::UnknownIdentifier {
        span: Span::new(20, 25),
        name: Name::from_raw(3),
        similar: None,
    };

    assert_eq!(p1, p2);
    assert_ne!(p1, p3);
}

#[test]
fn test_problem_hash() {
    use std::collections::HashSet;

    let p1 = SemanticProblem::UnknownIdentifier {
        span: Span::new(20, 25),
        name: Name::from_raw(1),
        similar: Some(Name::from_raw(2)),
    };

    let p2 = p1.clone();
    let p3 = SemanticProblem::UnusedVariable {
        span: Span::new(5, 10),
        name: Name::from_raw(4),
    };

    let mut set = HashSet::new();
    set.insert(p1);
    set.insert(p2); // duplicate
    set.insert(p3);

    assert_eq!(set.len(), 2);
}
