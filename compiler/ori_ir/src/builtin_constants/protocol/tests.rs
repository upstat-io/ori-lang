use super::*;

/// Semantic pin: every variant's arg count is pinned.
/// Adding/removing args to a protocol builtin changes its ABI contract.
#[test]
fn pin_arg_counts() {
    assert_eq!(ProtocolBuiltin::Index.arg_count(), 2);
    assert_eq!(ProtocolBuiltin::Iter.arg_count(), 1);
    assert_eq!(ProtocolBuiltin::IterNext.arg_count(), 2);
    assert_eq!(ProtocolBuiltin::IterDrop.arg_count(), 1);
    assert_eq!(ProtocolBuiltin::CollectSet.arg_count(), 1);
    assert_eq!(ProtocolBuiltin::Cast.arg_count(), 1);
}

/// Guard: adding a new `ProtocolBuiltin` variant without updating these
/// tests causes `ALL.len()` to change, failing this assertion.
///
/// This is defense-in-depth: `arg_ownership()`, `name()`, `is_intercepted()`,
/// and `arg_count()` all use exhaustive match (no `_` catch-all), so Rust's
/// compiler enforces coverage at compile time. This test-time guard catches
/// the case where match arms are updated but `ALL` or test assertions are not.
#[test]
fn all_variants_covered() {
    assert_eq!(ProtocolBuiltin::ALL.len(), 6);
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
fn cast_ownership_is_borrowed() {
    // `value as Target` reads the source value; the caller retains
    // ownership and emits the dec — the cast never consumes.
    let ownership = ProtocolBuiltin::Cast.arg_ownership();
    assert_eq!(ownership.len(), 1);
    assert_eq!(ownership[0], ProtocolArgOwnership::Borrowed);
}

#[test]
fn cast_name_round_trips_through_registry() {
    // Negative pin for the raw-literal desync class: the producer
    // (ARC lowering) and consumer (LLVM intercept) both resolve the
    // marker through the registry, so a rename cannot half-apply.
    assert_eq!(ProtocolBuiltin::Cast.name(), "__cast");
    assert_eq!(
        ProtocolBuiltin::from_name("__cast"),
        Some(ProtocolBuiltin::Cast)
    );
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
    assert!(ProtocolBuiltin::Cast.is_intercepted());
}

#[test]
fn is_intercepted_exhaustive() {
    // Ensure every variant has a defined interception status.
    for &pb in ProtocolBuiltin::ALL {
        // Just calling is_intercepted() proves the match is exhaustive.
        let _ = pb.is_intercepted();
    }
}

/// Regression pin (codegen-interception mechanism): `from_name` recognizes
/// every protocol-builtin emission name, and `ALL` covers exactly that name
/// set. This is the invariant that keeps a builtin `Apply` (`__cast` /
/// `__index` / `__iter_next` / `__collect_set` / `iter` / `ori_iter_drop`) in
/// a generic body OFF the user-function mono-dispatch path — codegen
/// intercepts it via `from_name`, so it never surfaces an E5001 missing-mono.
#[test]
fn from_name_recognizes_every_emission_name() {
    let emission_names = [
        "__index",
        "iter",
        "__iter_next",
        "ori_iter_drop",
        "__collect_set",
        "__cast",
    ];
    for name in emission_names {
        let Some(resolved) = ProtocolBuiltin::from_name(name) else {
            panic!("from_name should recognize emission name {name}")
        };
        // The round-trip is exact: the resolved variant re-emits the same name.
        assert_eq!(resolved.name(), name);
    }

    // `ALL` covers exactly the recognized emission names — no variant is
    // absent from `ALL` (which would leave its name on the mono path) and no
    // `ALL` member emits a name `from_name` cannot resolve.
    assert_eq!(ProtocolBuiltin::ALL.len(), emission_names.len());
    for &pb in ProtocolBuiltin::ALL {
        assert!(
            emission_names.contains(&pb.name()),
            "ALL member {pb:?} emits unrecognized name {}",
            pb.name()
        );
    }
}
