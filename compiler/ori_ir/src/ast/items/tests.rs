use std::path::PathBuf;

use super::ImportCycleGuard;

#[test]
fn would_cycle_tracks_in_progress_stack() {
    let mut guard = ImportCycleGuard::new();
    let a = PathBuf::from("/a.ori");
    let b = PathBuf::from("/b.ori");

    assert!(!guard.would_cycle(&a));
    guard
        .start_loading(a.clone())
        .unwrap_or_else(|c| panic!("unexpected cycle: {c:?}"));
    assert!(guard.would_cycle(&a));
    assert!(!guard.would_cycle(&b));

    guard
        .start_loading(b.clone())
        .unwrap_or_else(|c| panic!("unexpected cycle: {c:?}"));
    assert!(guard.would_cycle(&b));

    guard.finish_loading(&b);
    assert!(!guard.would_cycle(&b), "b popped off the in-progress stack");
    assert!(guard.is_visited(&b), "b marked visited after finishing");
}

#[test]
fn start_loading_reports_cycle_path() {
    let mut guard = ImportCycleGuard::new();
    let a = PathBuf::from("/a.ori");

    guard
        .start_loading(a.clone())
        .unwrap_or_else(|c| panic!("unexpected cycle: {c:?}"));
    let err = guard
        .start_loading(a.clone())
        .expect_err("re-entering an in-progress path must report a cycle");
    assert_eq!(err, vec![a.clone(), a]);
}

/// Self-import is a 1-cycle: re-entering the same path immediately after
/// starting it must report a cycle of length 2 (the path repeated).
#[test]
fn self_import_is_a_one_cycle() {
    let mut guard = ImportCycleGuard::new();
    let a = PathBuf::from("/a.ori");

    guard
        .start_loading(a.clone())
        .unwrap_or_else(|c| panic!("unexpected cycle: {c:?}"));
    let err = guard
        .start_loading(a.clone())
        .expect_err("self-import must cycle");
    assert_eq!(err.len(), 2);
    assert_eq!(err[0], a);
    assert_eq!(err[1], a);
}

/// Diamond import graph (A -> B, A -> C, B -> D, C -> D) is NOT a cycle: D
/// is reached twice via disjoint paths, but never while D itself is
/// in-progress. A naive one-set guard would false-positive on D's second
/// visit; the two-set (in-progress vs visited) discipline must not.
#[test]
fn diamond_graph_is_not_a_false_positive_cycle() {
    let mut guard = ImportCycleGuard::new();
    let a = PathBuf::from("/a.ori");
    let b = PathBuf::from("/b.ori");
    let c = PathBuf::from("/c.ori");
    let d = PathBuf::from("/d.ori");

    guard
        .start_loading(a)
        .unwrap_or_else(|cyc| panic!("unexpected cycle: {cyc:?}"));

    guard
        .start_loading(b)
        .unwrap_or_else(|cyc| panic!("unexpected cycle: {cyc:?}"));
    guard
        .start_loading(d.clone())
        .unwrap_or_else(|cyc| panic!("unexpected cycle: {cyc:?}"));
    guard.finish_loading(&d);
    guard.finish_loading(&PathBuf::from("/b.ori"));

    assert!(!guard.would_cycle(&d), "d finished loading via the B path");
    assert!(guard.is_visited(&d));

    guard
        .start_loading(c)
        .unwrap_or_else(|cyc| panic!("unexpected cycle: {cyc:?}"));
    // D is visited (not in-progress) — a second load through the C path
    // must be recognized as already-resolved, not a cycle.
    let result = guard.start_loading(d);
    assert!(
        result.is_ok(),
        "diamond re-visit of a fully-visited node must not be reported as a cycle: {result:?}"
    );
}
