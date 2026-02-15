use super::*;

#[test]
fn context_descriptions() {
    assert_eq!(
        ContextKind::IfCondition.describe(),
        "in the condition of this if expression"
    );

    assert_eq!(
        ContextKind::ListElement { index: 0 }.describe(),
        "in the 1st element of this list"
    );

    assert_eq!(
        ContextKind::ListElement { index: 2 }.describe(),
        "in the 3rd element of this list"
    );

    assert_eq!(
        ContextKind::MatchArm { arm_index: 0 }.describe(),
        "in the 1st match arm"
    );

    assert_eq!(
        ContextKind::BinaryOpLeft { op: "+" }.describe(),
        "in the left operand of `+`"
    );
}

#[test]
fn context_expectation_reasons() {
    assert_eq!(
        ContextKind::IfCondition.expectation_reason(),
        "if conditions must be bool"
    );

    assert_eq!(
        ContextKind::ListElement { index: 0 }.expectation_reason(),
        "all list elements must have the same type"
    );
}

#[test]
fn context_category_checks() {
    assert!(ContextKind::IfCondition.expects_bool());
    assert!(ContextKind::LoopCondition.expects_bool());
    assert!(!ContextKind::ListElement { index: 0 }.expects_bool());

    assert!(ContextKind::IfCondition.is_control_flow());
    assert!(!ContextKind::ListElement { index: 0 }.is_control_flow());

    assert!(ContextKind::FunctionArgument {
        func_name: None,
        arg_index: 0,
        param_name: None,
    }
    .is_function_call());
}
