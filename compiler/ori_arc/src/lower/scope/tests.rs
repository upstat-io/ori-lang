use ori_ir::Name;
use ori_types::Idx;
use rustc_hash::FxHashMap;

use crate::ir::ArcVarId;

use super::*;

fn name(n: u32) -> Name {
    Name::from_raw(n)
}

#[test]
fn empty_scope_lookup_returns_none() {
    let scope = ArcScope::new();
    assert!(scope.lookup(name(1)).is_none());
}

#[test]
fn bind_and_lookup() {
    let mut scope = ArcScope::new();
    let var = ArcVarId::new(0);
    scope.bind(name(1), var);
    assert_eq!(scope.lookup(name(1)), Some(var));
    assert!(!scope.is_mutable(name(1)));
}

#[test]
fn bind_mutable_and_lookup() {
    let mut scope = ArcScope::new();
    let var = ArcVarId::new(0);
    scope.bind_mutable(name(1), var);
    assert_eq!(scope.lookup(name(1)), Some(var));
    assert!(scope.is_mutable(name(1)));
}

#[test]
fn child_scope_inherits_bindings() {
    let mut parent = ArcScope::new();
    parent.bind(name(1), ArcVarId::new(0));

    let child = parent.clone();
    assert_eq!(child.lookup(name(1)), Some(ArcVarId::new(0)));
}

#[test]
fn child_scope_does_not_affect_parent() {
    let mut parent = ArcScope::new();
    parent.bind(name(1), ArcVarId::new(0));

    let mut child = parent.clone();
    child.bind(name(2), ArcVarId::new(1));

    assert!(parent.lookup(name(2)).is_none());
    assert_eq!(child.lookup(name(2)), Some(ArcVarId::new(1)));
}

#[test]
fn mutable_bindings_iterator() {
    let mut scope = ArcScope::new();
    scope.bind(name(1), ArcVarId::new(0));
    scope.bind_mutable(name(2), ArcVarId::new(1));
    scope.bind_mutable(name(3), ArcVarId::new(2));

    let mut muts: Vec<_> = scope.mutable_bindings().collect();
    muts.sort_by_key(|(n, _)| n.raw());
    assert_eq!(muts.len(), 2);
}

#[test]
fn rebind_mutable_creates_new_version() {
    let mut scope = ArcScope::new();
    scope.bind_mutable(name(1), ArcVarId::new(0));

    // Rebind simulates `x = new_value` in SSA.
    scope.bind_mutable(name(1), ArcVarId::new(5));
    assert_eq!(scope.lookup(name(1)), Some(ArcVarId::new(5)));
}

#[test]
fn merge_mutable_vars_detects_changes() {
    let mut builder = ArcIrBuilder::new();
    let merge_bb = builder.new_block();

    let mut pre_scope = ArcScope::new();
    pre_scope.bind_mutable(name(1), ArcVarId::new(10));

    // Branch 1: changed.
    let mut branch1 = pre_scope.clone();
    // Need to allocate vars so the builder's var counter is past 10.
    for _ in 0..11 {
        builder.fresh_var(Idx::INT);
    }
    branch1.bind_mutable(name(1), ArcVarId::new(11));

    // Branch 2: unchanged.
    let branch2 = pre_scope.clone();

    let mut var_types = FxHashMap::default();
    var_types.insert(name(1), Idx::INT);

    let rebindings = merge_mutable_vars(
        &mut builder,
        merge_bb,
        &pre_scope,
        &[branch1, branch2],
        &var_types,
    );

    assert_eq!(rebindings.len(), 1);
    assert_eq!(rebindings[0].0, name(1));
}

#[test]
fn merge_mutable_vars_no_changes_produces_empty() {
    let mut builder = ArcIrBuilder::new();
    let merge_bb = builder.new_block();

    let mut pre_scope = ArcScope::new();
    pre_scope.bind_mutable(name(1), ArcVarId::new(0));
    builder.fresh_var(Idx::INT); // Ensure var 0 exists.

    let branch1 = pre_scope.clone();
    let branch2 = pre_scope.clone();

    let var_types = FxHashMap::default();
    let rebindings = merge_mutable_vars(
        &mut builder,
        merge_bb,
        &pre_scope,
        &[branch1, branch2],
        &var_types,
    );

    assert!(rebindings.is_empty());
}
