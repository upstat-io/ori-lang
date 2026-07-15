//! Unit tests for the operator-gate substitution extraction.

use ori_ir::Name;
use rustc_hash::FxHashMap;

use super::resolve_under_subst;
use crate::infer::expr::calls::match_self_type;
use crate::{Idx, Pool};

fn name(s: &str) -> Name {
    Name::from_raw(
        s.bytes()
            .fold(0u32, |acc, b| acc.wrapping_add(u32::from(b))),
    )
}

// Manual impl `impl<T> Pair<T>: Eq` carries `self_type = Applied(Pair, [Named(T)])`;
// `match_self_type` binds the param to the concrete arg.
#[test]
fn extract_subst_applied_self_type_binds_via_match() {
    let mut pool = Pool::new();
    let (t, pair) = (name("T"), name("Pair"));
    let t_idx = pool.named(t);
    let self_ty = pool.applied(pair, &[t_idx]);
    let target = pool.applied(pair, &[Idx::INT]);

    let subst = match_self_type(&pool, self_ty, target, &[t]).expect("should match");
    assert_eq!(subst.get(&t), Some(&Idx::INT));
}

// Non-generic impl (no type params): empty subst, applicable.
#[test]
fn match_self_type_non_generic_returns_empty_subst() {
    let mut pool = Pool::new();
    let p = name("P");
    let self_ty = pool.named(p);
    let target = pool.named(p);

    let subst = match_self_type(&pool, self_ty, target, &[]).expect("applicable, empty subst");
    assert!(subst.is_empty());
}

#[test]
fn bare_generic_owner_does_not_match_applied_receiver() {
    let mut pool = Pool::new();
    let (t, pair) = (name("T"), name("Pair"));
    let pattern = pool.named(pair);
    let receiver = pool.applied(pair, &[Idx::INT]);

    assert!(match_self_type(&pool, pattern, receiver, &[t]).is_none());
}

#[test]
fn applied_generic_owner_rejects_receiver_arity_drift() {
    let mut pool = Pool::new();
    let (t, pair) = (name("T"), name("Pair"));
    let t_idx = pool.named(t);
    let pattern = pool.applied(pair, &[t_idx]);
    let receiver = pool.applied(pair, &[Idx::INT, Idx::STR]);

    assert!(match_self_type(&pool, pattern, receiver, &[t]).is_none());
}

// A receiver whose nominal head differs from the registered pattern does not match.
#[test]
fn match_self_type_no_alignment_returns_none() {
    let mut pool = Pool::new();
    let (t, pair, other) = (name("T"), name("Pair"), name("Other"));
    let t_idx = pool.named(t);
    let self_ty = pool.applied(pair, &[t_idx]);
    let target = pool.applied(other, &[Idx::INT]);

    assert!(match_self_type(&pool, self_ty, target, &[t]).is_none());
}

// `where T: Eq` — a direct binder resolves to its concrete arg.
#[test]
fn resolve_under_subst_direct_binder_to_concrete() {
    let mut pool = Pool::new();
    let t = name("T");
    let ty = pool.named(t);
    let mut subst = FxHashMap::default();
    subst.insert(t, Idx::INT);

    assert_eq!(resolve_under_subst(&pool, ty, &subst), Some(Idx::INT));
}

// A concrete `Named` constrained type passes through unchanged.
#[test]
fn resolve_under_subst_concrete_passes_through() {
    let mut pool = Pool::new();
    let p = name("P");
    let ty = pool.named(p);
    let subst = FxHashMap::default();

    assert_eq!(resolve_under_subst(&pool, ty, &subst), Some(ty));
}

// `where Box<T>: Eq` — a nested generic referencing a binder cannot be
// substituted in place, so the helper returns `None` (caller skips the bound).
#[test]
fn resolve_under_subst_nested_binder_returns_none() {
    let mut pool = Pool::new();
    let (t, boxn) = (name("T"), name("Box"));
    let t_idx = pool.named(t);
    let nested = pool.applied(boxn, &[t_idx]);
    let mut subst = FxHashMap::default();
    subst.insert(t, Idx::INT);

    assert_eq!(resolve_under_subst(&pool, nested, &subst), None);
}

// A fully-concrete `Applied` constrained type (no binder) passes through.
#[test]
fn resolve_under_subst_concrete_applied_passes_through() {
    let mut pool = Pool::new();
    let boxn = name("Box");
    let concrete = pool.applied(boxn, &[Idx::INT]);
    let mut subst = FxHashMap::default();
    subst.insert(name("T"), Idx::INT);

    assert_eq!(resolve_under_subst(&pool, concrete, &subst), Some(concrete));
}
