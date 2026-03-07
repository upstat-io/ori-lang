use super::*;
use crate::tags::{DeiPropagation, MethodKind, Ownership, ReturnTag, TypeTag};

#[test]
fn method_def_is_const_constructible() {
    const METHOD: MethodDef = MethodDef {
        name: "abs",
        receiver: Ownership::Borrow,
        params: &[],
        returns: ReturnTag::SelfType,
        trait_name: None,
        pure: true,
        backend_required: true,
        kind: MethodKind::Instance,
        dei_only: false,
        dei_propagation: DeiPropagation::NotApplicable,
    };
    let m = METHOD;
    assert_eq!(m.name, "abs");
    assert_eq!(m.receiver, Ownership::Borrow);
    assert!(m.params.is_empty());
    assert_eq!(m.returns, ReturnTag::SelfType);
    assert_eq!(m.trait_name, None);
    assert!(m.pure);
    assert!(m.backend_required);
    assert_eq!(m.kind, MethodKind::Instance);
    assert!(!m.dei_only);
    assert_eq!(m.dei_propagation, DeiPropagation::NotApplicable);
}

#[test]
fn method_def_in_static_slice() {
    static METHODS: &[MethodDef] = &[
        MethodDef {
            name: "to_str",
            receiver: Ownership::Borrow,
            params: &[],
            returns: ReturnTag::Concrete(TypeTag::Str),
            trait_name: Some("Printable"),
            pure: true,
            backend_required: true,
            kind: MethodKind::Instance,
            dei_only: false,
            dei_propagation: DeiPropagation::NotApplicable,
        },
        MethodDef {
            name: "clone",
            receiver: Ownership::Borrow,
            params: &[],
            returns: ReturnTag::SelfType,
            trait_name: Some("Clone"),
            pure: true,
            backend_required: true,
            kind: MethodKind::Instance,
            dei_only: false,
            dei_propagation: DeiPropagation::NotApplicable,
        },
    ];
    assert_eq!(METHODS.len(), 2);
    assert_eq!(METHODS[0].name, "to_str");
    assert_eq!(METHODS[1].name, "clone");
}

#[test]
fn param_def_is_const_constructible() {
    const PARAM: ParamDef = ParamDef {
        name: "value",
        ty: ReturnTag::Concrete(TypeTag::Int),
        ownership: Ownership::Owned,
    };
    assert_eq!(PARAM.name, "value");
    assert_eq!(PARAM.ty, ReturnTag::Concrete(TypeTag::Int));
    assert_eq!(PARAM.ownership, Ownership::Owned);
}

#[test]
fn method_def_with_params() {
    static PARAMS: &[ParamDef] = &[
        ParamDef {
            name: "key",
            ty: ReturnTag::KeyType,
            ownership: Ownership::Owned,
        },
        ParamDef {
            name: "value",
            ty: ReturnTag::ValueType,
            ownership: Ownership::Owned,
        },
    ];
    const METHOD: MethodDef = MethodDef {
        name: "insert",
        receiver: Ownership::Borrow,
        params: PARAMS,
        returns: ReturnTag::SelfType,
        trait_name: None,
        pure: true,
        backend_required: true,
        kind: MethodKind::Instance,
        dei_only: false,
        dei_propagation: DeiPropagation::NotApplicable,
    };
    assert_eq!(METHOD.params.len(), 2);
    assert_eq!(METHOD.params[0].name, "key");
    assert_eq!(METHOD.params[1].name, "value");
}

#[test]
fn method_def_size_within_cache_line() {
    assert!(
        std::mem::size_of::<MethodDef>() <= 64,
        "MethodDef should fit within a cache line (64 bytes), got {}",
        std::mem::size_of::<MethodDef>()
    );
}
