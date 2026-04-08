use super::*;

#[test]
fn all_variants_covered() {
    assert_eq!(ProtocolBuiltin::ALL.len(), 5);
    for &pb in ProtocolBuiltin::ALL {
        assert!(!pb.name().is_empty());
        assert!(ProtocolBuiltin::from_name(pb.name()) == Some(pb));
        assert_eq!(pb.arg_ownership().len(), pb.arg_count());
    }
}

#[test]
fn from_name_returns_none_for_unknown() {
    assert!(ProtocolBuiltin::from_name("unknown").is_none());
    assert!(ProtocolBuiltin::from_name("ori_print").is_none());
}

#[test]
fn index_ownership_is_all_borrowed() {
    let ownership = ProtocolBuiltin::Index.arg_ownership();
    assert!(ownership
        .iter()
        .all(|o| *o == ProtocolArgOwnership::Borrowed));
}

#[test]
fn iter_next_ownership_is_owned_borrowed() {
    let ownership = ProtocolBuiltin::IterNext.arg_ownership();
    assert_eq!(ownership[0], ProtocolArgOwnership::Owned);
    assert_eq!(ownership[1], ProtocolArgOwnership::Borrowed);
}

#[test]
fn iter_ownership_is_borrowed() {
    let ownership = ProtocolBuiltin::Iter.arg_ownership();
    assert_eq!(ownership.len(), 1);
    assert_eq!(ownership[0], ProtocolArgOwnership::Borrowed);
}

#[test]
fn iter_drop_ownership_is_owned() {
    // `ori_iter_drop` consumes the iterator handle (frees
    // the Box-allocated state). Before the fix, iterators were trivial
    // so ARC never emitted scope-exit drops and the Borrowed contract
    // was harmless; now that iterators are non-trivial, the explicit
    // for-loop `ori_iter_drop` call must be marked as consuming so
    // borrow inference does not also insert a second scope-exit
    // `RcDec`, which would double-free the iterator state.
    let ownership = ProtocolBuiltin::IterDrop.arg_ownership();
    assert_eq!(ownership.len(), 1);
    assert_eq!(ownership[0], ProtocolArgOwnership::Owned);
}

#[test]
fn collect_set_ownership_is_owned() {
    let ownership = ProtocolBuiltin::CollectSet.arg_ownership();
    assert_eq!(ownership[0], ProtocolArgOwnership::Owned);
}

#[test]
fn is_intercepted_matches_dispatch() {
    // Iter and IterDrop go through normal function dispatch, not protocol intercept.
    assert!(!ProtocolBuiltin::Iter.is_intercepted());
    assert!(!ProtocolBuiltin::IterDrop.is_intercepted());
    // All others are intercepted by try_emit_protocol().
    assert!(ProtocolBuiltin::Index.is_intercepted());
    assert!(ProtocolBuiltin::IterNext.is_intercepted());
    assert!(ProtocolBuiltin::CollectSet.is_intercepted());
}

#[test]
fn is_intercepted_exhaustive() {
    // Ensure every variant has a defined interception status.
    for &pb in ProtocolBuiltin::ALL {
        // Just calling is_intercepted() proves the match is exhaustive.
        let _ = pb.is_intercepted();
    }
}
