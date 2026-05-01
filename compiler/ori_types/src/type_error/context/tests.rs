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

// Regression:
// HigherOrderClosureReturn distinguishes a closure return position inside a
// higher-order iterator adapter (e.g., flat_map) from a generic LambdaReturn,
// because the closure return carries a structural Iterator<U> requirement
// imposed by the enclosing adapter.

#[test]
fn higher_order_closure_return_describe_names_adapter() {
    assert_eq!(
        ContextKind::HigherOrderClosureReturn {
            adapter_name: "flat_map",
        }
        .describe(),
        "in the closure return of `flat_map`"
    );
}

#[test]
fn higher_order_closure_return_expectation_reason_demands_iterator() {
    assert_eq!(
        ContextKind::HigherOrderClosureReturn {
            adapter_name: "flat_map",
        }
        .expectation_reason(),
        "closure must return an iterator type"
    );
}

#[test]
fn higher_order_closure_return_is_function_call_false() {
    // The closure-return position is NOT a function-call site — the enclosing
    // method call (flat_map itself) is the function call; the closure body is
    // a separately-typed expression whose return shape is being constrained.
    assert!(!ContextKind::HigherOrderClosureReturn {
        adapter_name: "flat_map",
    }
    .is_function_call());
}

#[test]
fn higher_order_closure_return_is_control_flow_false() {
    // Closure-return is structural shape enforcement, not control flow.
    assert!(!ContextKind::HigherOrderClosureReturn {
        adapter_name: "flat_map",
    }
    .is_control_flow());
}

#[test]
fn higher_order_closure_return_expects_bool_false() {
    // The expected type is Iterator<U>, never bool.
    assert!(!ContextKind::HigherOrderClosureReturn {
        adapter_name: "flat_map",
    }
    .expects_bool());
}
