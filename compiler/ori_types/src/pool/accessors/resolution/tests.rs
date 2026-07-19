use super::*;
use ori_ir::Name;

/// INVARIANT: `resolve_fully` terminates on a self-similar nested `Applied`
/// type (`Wrap<Wrap<int>>` resolving its inner `Wrap<int>`) whose registered
/// concrete-instantiation key collides on name + arity with the inner type
/// being resolved — the in-progress guard breaks the self-referential match
/// candidate, and a type with no registered resolution of its own returns
/// its own unresolved leaf.
#[test]
fn resolve_fully_terminates_on_self_similar_nested_applied() {
    let mut pool = Pool::new();
    let wrap = Name::from_raw(2075);
    // inner = Wrap<int>; outer = Wrap<Wrap<int>>; both share name + arity 1.
    let inner = pool.applied(wrap, &[Idx::INT]);
    let outer = pool.applied(wrap, &[inner]);
    // Register only the OUTER instantiation as a resolution key. Resolving the
    // INNER type then scans keys, finds OUTER (name+arity match), and recurses
    // on OUTER's nested arg (== inner) — the self-referential cycle.
    pool.set_resolution(outer, Idx::INT);
    // Must return (not overflow). Inner has no registered resolution of its
    // own, so the self-referential candidate is rejected and the leaf returns.
    assert_eq!(pool.resolve_fully(inner), inner);
}

/// Negative pin: the `resolve_applied_via_matching_args` path still resolves
/// a generic instantiation whose query `Idx` differs structurally from the
/// registered key but whose args resolve-equal — the cycle guard does NOT
/// block legitimate match-based resolution. The query has NO direct
/// resolution (so `resolve()` returns `None` and the matching path runs);
/// the registered key carries an arg that must itself be resolved to match.
#[test]
fn resolve_fully_matches_applied_with_resolution_equal_args() {
    let mut pool = Pool::new();
    let wrap = Name::from_raw(2076);
    // A named alias that resolves to int — forces the matching loop to
    // resolve the key's arg before comparing.
    let alias = pool.named(Name::from_raw(2077));
    pool.set_resolution(alias, Idx::INT);
    // key = Wrap<alias> (alias resolves to int); resolves to the FLOAT marker.
    let key = pool.applied(wrap, &[alias]);
    pool.set_resolution(key, Idx::FLOAT);
    // query = Wrap<int> — structurally distinct from `key` (int vs alias),
    // has no direct resolution of its own, so it routes through matching.
    let query = pool.applied(wrap, &[Idx::INT]);
    assert_eq!(pool.resolve_fully(query), Idx::FLOAT);
}
