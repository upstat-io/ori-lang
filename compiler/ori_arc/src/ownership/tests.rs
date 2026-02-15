use ori_ir::Name;
use ori_types::Idx;

use crate::ir::ArcVarId;

use super::*;

#[test]
fn ownership_inequality() {
    assert_ne!(Ownership::Borrowed, Ownership::Owned);
}

#[test]
fn ownership_is_copy() {
    let o = Ownership::Borrowed;
    let o2 = o;
    // Both are valid — Copy semantics.
    assert_eq!(o, o2);
}

// DerivedOwnership tests

#[test]
fn derived_ownership_variants() {
    let owned = DerivedOwnership::Owned;
    let borrowed = DerivedOwnership::BorrowedFrom(ArcVarId::new(3));
    let fresh = DerivedOwnership::Fresh;
    assert_ne!(owned, borrowed);
    assert_ne!(owned, fresh);
    assert_ne!(borrowed, fresh);
}

#[test]
fn derived_ownership_is_copy() {
    let d = DerivedOwnership::BorrowedFrom(ArcVarId::new(7));
    let d2 = d;
    assert_eq!(d, d2);
}

#[test]
fn derived_borrowed_from_carries_source() {
    let src = ArcVarId::new(42);
    let d = DerivedOwnership::BorrowedFrom(src);
    match d {
        DerivedOwnership::BorrowedFrom(v) => assert_eq!(v, src),
        _ => panic!("expected BorrowedFrom"),
    }
}

#[test]
fn derived_ownership_equality_by_source() {
    let a = DerivedOwnership::BorrowedFrom(ArcVarId::new(1));
    let b = DerivedOwnership::BorrowedFrom(ArcVarId::new(1));
    let c = DerivedOwnership::BorrowedFrom(ArcVarId::new(2));
    assert_eq!(a, b);
    assert_ne!(a, c);
}

#[test]
fn annotated_param_construction() {
    let param = AnnotatedParam {
        name: Name::from_raw(1),
        ty: Idx::INT,
        ownership: Ownership::Borrowed,
    };
    assert_eq!(param.name, Name::from_raw(1));
    assert_eq!(param.ty, Idx::INT);
    assert_eq!(param.ownership, Ownership::Borrowed);
}

#[test]
fn annotated_param_equality() {
    let a = AnnotatedParam {
        name: Name::from_raw(1),
        ty: Idx::INT,
        ownership: Ownership::Borrowed,
    };
    let b = AnnotatedParam {
        name: Name::from_raw(1),
        ty: Idx::INT,
        ownership: Ownership::Borrowed,
    };
    let c = AnnotatedParam {
        name: Name::from_raw(1),
        ty: Idx::INT,
        ownership: Ownership::Owned,
    };
    assert_eq!(a, b);
    assert_ne!(a, c);
}

#[test]
fn annotated_sig_construction() {
    let sig = AnnotatedSig {
        params: vec![
            AnnotatedParam {
                name: Name::from_raw(1),
                ty: Idx::STR,
                ownership: Ownership::Borrowed,
            },
            AnnotatedParam {
                name: Name::from_raw(2),
                ty: Idx::INT,
                ownership: Ownership::Owned,
            },
        ],
        return_type: Idx::BOOL,
    };
    assert_eq!(sig.params.len(), 2);
    assert_eq!(sig.params[0].ownership, Ownership::Borrowed);
    assert_eq!(sig.params[1].ownership, Ownership::Owned);
    assert_eq!(sig.return_type, Idx::BOOL);
}

#[test]
fn annotated_sig_empty_params() {
    let sig = AnnotatedSig {
        params: vec![],
        return_type: Idx::UNIT,
    };
    assert!(sig.params.is_empty());
    assert_eq!(sig.return_type, Idx::UNIT);
}
