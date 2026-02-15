use super::*;

#[test]
fn ordinal_formatting() {
    assert_eq!(ordinal(1), "1st");
    assert_eq!(ordinal(2), "2nd");
    assert_eq!(ordinal(3), "3rd");
    assert_eq!(ordinal(4), "4th");
    assert_eq!(ordinal(11), "11th");
    assert_eq!(ordinal(12), "12th");
    assert_eq!(ordinal(13), "13th");
    assert_eq!(ordinal(21), "21st");
    assert_eq!(ordinal(22), "22nd");
    assert_eq!(ordinal(23), "23rd");
    assert_eq!(ordinal(100), "100th");
    assert_eq!(ordinal(101), "101st");
    assert_eq!(ordinal(111), "111th");
}

#[test]
fn expected_no_expectation() {
    let exp = Expected::no_expectation(Idx::INT);
    assert_eq!(exp.ty, Idx::INT);
    assert!(!exp.has_expectation());
}

#[test]
fn expected_from_annotation() {
    let name = Name::from_raw(1);
    let span = Span::new(0, 10);
    let exp = Expected::from_annotation(Idx::STR, name, span);
    assert_eq!(exp.ty, Idx::STR);
    assert!(exp.has_expectation());
    assert!(matches!(exp.origin, ExpectedOrigin::Annotation { .. }));
}

#[test]
fn sequence_kind_descriptions() {
    assert_eq!(SequenceKind::ListLiteral.description(), "list literal");
    assert_eq!(SequenceKind::MatchArms.description(), "match arms");
    assert_eq!(SequenceKind::IfBranches.description(), "if branches");
}

#[test]
fn previous_in_sequence_description() {
    let origin = ExpectedOrigin::PreviousInSequence {
        previous_span: Span::new(0, 5),
        current_index: 2, // 3rd element (0-indexed)
        sequence_kind: SequenceKind::ListLiteral,
    };
    let desc = origin.describe();
    assert!(desc.contains("3rd"));
    assert!(desc.contains("list"));
}
