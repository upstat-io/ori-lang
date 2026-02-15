use super::*;
use crate::{ExprId, MatchPattern};

#[test]
fn test_function_seq_name_all_variants() {
    // Verify all 4 FunctionSeq variants return correct names
    let run = FunctionSeq::Run {
        pre_checks: CheckRange::EMPTY,
        bindings: SeqBindingRange::EMPTY,
        result: ExprId::new(0),
        post_checks: CheckRange::EMPTY,
        span: Span::new(0, 10),
    };
    assert_eq!(run.name(), "run");

    let try_seq = FunctionSeq::Try {
        bindings: SeqBindingRange::EMPTY,
        result: ExprId::new(0),
        span: Span::new(0, 10),
    };
    assert_eq!(try_seq.name(), "try");

    let match_seq = FunctionSeq::Match {
        scrutinee: ExprId::new(0),
        arms: ArmRange::EMPTY,
        span: Span::new(0, 10),
    };
    assert_eq!(match_seq.name(), "match");

    let for_pattern = FunctionSeq::ForPattern {
        over: ExprId::new(0),
        map: None,
        arm: MatchArm {
            pattern: MatchPattern::Wildcard,
            guard: None,
            body: ExprId::new(1),
            span: Span::new(5, 10),
        },
        default: ExprId::new(2),
        span: Span::new(0, 15),
    };
    assert_eq!(for_pattern.name(), "for");
}

#[test]
fn test_function_seq_span_all_variants() {
    let run_span = Span::new(0, 10);
    let run = FunctionSeq::Run {
        pre_checks: CheckRange::EMPTY,
        bindings: SeqBindingRange::EMPTY,
        result: ExprId::new(0),
        post_checks: CheckRange::EMPTY,
        span: run_span,
    };
    assert_eq!(run.span(), run_span);

    let try_span = Span::new(5, 20);
    let try_seq = FunctionSeq::Try {
        bindings: SeqBindingRange::EMPTY,
        result: ExprId::new(0),
        span: try_span,
    };
    assert_eq!(try_seq.span(), try_span);

    let match_span = Span::new(10, 30);
    let match_seq = FunctionSeq::Match {
        scrutinee: ExprId::new(0),
        arms: ArmRange::EMPTY,
        span: match_span,
    };
    assert_eq!(match_seq.span(), match_span);

    let for_span = Span::new(15, 40);
    let for_pattern = FunctionSeq::ForPattern {
        over: ExprId::new(0),
        map: Some(ExprId::new(1)),
        arm: MatchArm {
            pattern: MatchPattern::Wildcard,
            guard: None,
            body: ExprId::new(2),
            span: Span::new(20, 35),
        },
        default: ExprId::new(3),
        span: for_span,
    };
    assert_eq!(for_pattern.span(), for_span);
}

#[test]
fn test_function_seq_spanned_trait() {
    use crate::Spanned;

    let run = FunctionSeq::Run {
        pre_checks: CheckRange::EMPTY,
        bindings: SeqBindingRange::EMPTY,
        result: ExprId::new(0),
        post_checks: CheckRange::EMPTY,
        span: Span::new(100, 200),
    };

    // Test that Spanned trait works correctly
    let spanned: &dyn Spanned = &run;
    assert_eq!(spanned.span(), Span::new(100, 200));
}
