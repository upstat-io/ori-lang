use super::*;

#[test]
fn context_descriptions() {
    assert_eq!(UnifyContext::TopLevel.description(), "types");
    assert_eq!(UnifyContext::param(0).description(), "function parameter");
    assert_eq!(
        UnifyContext::FunctionReturn.description(),
        "function return type"
    );
    assert_eq!(UnifyContext::tuple_elem(2).description(), "tuple element");
}

#[test]
fn error_display() {
    let err = UnifyError::ArityMismatch {
        expected: 2,
        found: 3,
        kind: ArityKind::Function,
    };
    assert_eq!(
        err.to_string(),
        "arity mismatch: expected 2 function parameters, found 3"
    );
}
