use ori_ir::Name;
use ori_types::Idx;

use crate::ir::{ArcBlock, ArcInstr, ArcTerminator, ArgOwnership};
use crate::test_helpers::{b, make_func_named, owned_param, v};

use super::*;

/// Shorthand for `Name::from_raw(n)` in tests.
fn n(raw: u32) -> Name {
    Name::from_raw(raw)
}

/// Build a minimal function with a given name and a single block
/// containing the provided instructions + a return terminator.
fn leaf_func(name: Name, var_count: usize) -> ArcFunction {
    make_func_named(
        name,
        vec![owned_param(0, Idx::INT)],
        Idx::INT,
        vec![ArcBlock {
            id: b(0),
            params: vec![],
            body: vec![],
            terminator: ArcTerminator::Return { value: v(0) },
        }],
        vec![Idx::INT; var_count],
    )
}

/// Build a function that calls `callee` via Apply.
fn calling_func(name: Name, callee: Name, var_count: usize) -> ArcFunction {
    make_func_named(
        name,
        vec![owned_param(0, Idx::INT)],
        Idx::INT,
        vec![ArcBlock {
            id: b(0),
            params: vec![],
            body: vec![ArcInstr::Apply {
                dst: v(1),
                ty: Idx::INT,
                func: callee,
                args: vec![v(0)],
                arg_ownership: vec![ArgOwnership::Owned],
            }],
            terminator: ArcTerminator::Return { value: v(1) },
        }],
        vec![Idx::INT; var_count],
    )
}

/// Build a function that calls `callee` via Invoke (potentially unwinding).
fn invoking_func(name: Name, callee: Name, var_count: usize) -> ArcFunction {
    make_func_named(
        name,
        vec![owned_param(0, Idx::INT)],
        Idx::INT,
        vec![
            ArcBlock {
                id: b(0),
                params: vec![],
                body: vec![],
                terminator: ArcTerminator::Invoke {
                    dst: v(1),
                    ty: Idx::INT,
                    func: callee,
                    args: vec![v(0)],
                    arg_ownership: vec![ArgOwnership::Owned],
                    normal: b(1),
                    unwind: b(2),
                },
            },
            ArcBlock {
                id: b(1),
                params: vec![],
                body: vec![],
                terminator: ArcTerminator::Return { value: v(1) },
            },
            ArcBlock {
                id: b(2),
                params: vec![],
                body: vec![],
                terminator: ArcTerminator::Resume,
            },
        ],
        vec![Idx::INT; var_count],
    )
}

/// Build a function that does `PartialApply` of `callee`.
fn partial_apply_func(name: Name, callee: Name, var_count: usize) -> ArcFunction {
    make_func_named(
        name,
        vec![owned_param(0, Idx::INT)],
        Idx::INT,
        vec![ArcBlock {
            id: b(0),
            params: vec![],
            body: vec![ArcInstr::PartialApply {
                dst: v(1),
                ty: Idx::INT,
                func: callee,
                args: vec![v(0)],
            }],
            terminator: ArcTerminator::Return { value: v(1) },
        }],
        vec![Idx::INT; var_count],
    )
}

/// Build a function that does `ApplyIndirect` (closure call).
fn indirect_call_func(name: Name, var_count: usize) -> ArcFunction {
    make_func_named(
        name,
        vec![owned_param(0, Idx::INT), owned_param(1, Idx::INT)],
        Idx::INT,
        vec![ArcBlock {
            id: b(0),
            params: vec![],
            body: vec![ArcInstr::ApplyIndirect {
                dst: v(2),
                ty: Idx::INT,
                closure: v(0),
                args: vec![v(1)],
            }],
            terminator: ArcTerminator::Return { value: v(2) },
        }],
        vec![Idx::INT; var_count],
    )
}

// Tests

#[test]
fn empty_graph() {
    let graph = CallGraph::build(&[]);
    assert!(graph.is_empty());
    assert_eq!(graph.len(), 0);
    assert_eq!(graph.functions().count(), 0);
}

#[test]
fn single_function_no_calls() {
    let a = n(1);
    let func = leaf_func(a, 1);
    let graph = CallGraph::build(&[func]);

    assert_eq!(graph.len(), 1);
    assert!(graph.callees_of(a).is_empty());
    assert!(graph.callers_of(a).is_empty());
    assert!(graph.is_leaf(a));
    assert!(!graph.is_recursive(a));
}

#[test]
fn direct_call_chain() {
    // A → B → C
    let (a, b_name, c) = (n(1), n(2), n(3));
    let funcs = vec![
        calling_func(a, b_name, 2),
        calling_func(b_name, c, 2),
        leaf_func(c, 1),
    ];
    let graph = CallGraph::build(&funcs);

    assert_eq!(graph.len(), 3);

    // A calls B
    assert!(graph.callees_of(a).contains(&b_name));
    assert_eq!(graph.callees_of(a).len(), 1);
    // B calls C
    assert!(graph.callees_of(b_name).contains(&c));
    assert_eq!(graph.callees_of(b_name).len(), 1);
    // C calls nothing
    assert!(graph.callees_of(c).is_empty());
    assert!(graph.is_leaf(c));

    // Reverse: B is called by A
    assert!(graph.callers_of(b_name).contains(&a));
    // C is called by B
    assert!(graph.callers_of(c).contains(&b_name));
    // A has no callers
    assert!(graph.callers_of(a).is_empty());

    assert!(!graph.is_recursive(a));
    assert!(!graph.is_recursive(b_name));
    assert!(!graph.is_recursive(c));
}

#[test]
fn mutual_recursion() {
    // A → B, B → A
    let (a, b_name) = (n(1), n(2));
    let funcs = vec![calling_func(a, b_name, 2), calling_func(b_name, a, 2)];
    let graph = CallGraph::build(&funcs);

    assert!(graph.callees_of(a).contains(&b_name));
    assert!(graph.callees_of(b_name).contains(&a));
    assert!(graph.callers_of(a).contains(&b_name));
    assert!(graph.callers_of(b_name).contains(&a));
}

#[test]
fn self_recursion() {
    // A → A
    let a = n(1);
    let func = calling_func(a, a, 2);
    let graph = CallGraph::build(&[func]);

    assert!(graph.is_recursive(a));
    assert!(graph.callees_of(a).contains(&a));
    assert!(graph.callers_of(a).contains(&a));
    assert!(!graph.is_leaf(a));
}

#[test]
fn partial_apply_tracked() {
    // A partially applies B
    let (a, b_name) = (n(1), n(2));
    let funcs = vec![partial_apply_func(a, b_name, 2), leaf_func(b_name, 1)];
    let graph = CallGraph::build(&funcs);

    assert!(graph.callees_of(a).contains(&b_name));
    assert!(graph.callers_of(b_name).contains(&a));
}

#[test]
fn invoke_tracked() {
    // A invokes B (potentially unwinding call)
    let (a, b_name) = (n(1), n(2));
    let funcs = vec![invoking_func(a, b_name, 2), leaf_func(b_name, 1)];
    let graph = CallGraph::build(&funcs);

    assert!(graph.callees_of(a).contains(&b_name));
    assert!(graph.callers_of(b_name).contains(&a));
}

#[test]
fn indirect_call_ignored() {
    // A does ApplyIndirect — should NOT add any callees
    let a = n(1);
    let func = indirect_call_func(a, 3);
    let graph = CallGraph::build(&[func]);

    assert!(graph.callees_of(a).is_empty());
    assert!(graph.is_leaf(a));
}

#[test]
fn external_callee_in_set() {
    // A calls "ext" which is not in the function set
    let a = n(1);
    let ext = n(99);
    let func = calling_func(a, ext, 2);
    let graph = CallGraph::build(&[func]);

    // ext appears as a callee of A
    assert!(graph.callees_of(a).contains(&ext));
    // But ext is NOT in the function set
    assert!(!graph.functions().any(|f| f == ext));
    // A is a leaf because its only callee is external
    assert!(graph.is_leaf(a));
}

#[test]
fn reverse_index_consistent() {
    // For every edge A→B in callees, verify B→A in callers.
    let (a, b_name, c) = (n(1), n(2), n(3));

    // A calls both B and C; B calls C
    let a_func = make_func_named(
        a,
        vec![owned_param(0, Idx::INT)],
        Idx::INT,
        vec![ArcBlock {
            id: b(0),
            params: vec![],
            body: vec![
                ArcInstr::Apply {
                    dst: v(1),
                    ty: Idx::INT,
                    func: b_name,
                    args: vec![v(0)],
                    arg_ownership: vec![ArgOwnership::Owned],
                },
                ArcInstr::Apply {
                    dst: v(2),
                    ty: Idx::INT,
                    func: c,
                    args: vec![v(1)],
                    arg_ownership: vec![ArgOwnership::Owned],
                },
            ],
            terminator: ArcTerminator::Return { value: v(2) },
        }],
        vec![Idx::INT; 3],
    );

    let funcs = vec![a_func, calling_func(b_name, c, 2), leaf_func(c, 1)];
    let graph = CallGraph::build(&funcs);

    // Check every forward edge has a corresponding reverse edge.
    for caller_name in graph.functions() {
        for callee_name in graph.callees_of(caller_name) {
            assert!(
                graph.callers_of(*callee_name).contains(&caller_name),
                "forward edge {caller_name:?}→{callee_name:?} has no reverse entry"
            );
        }
    }

    // Check every reverse edge has a corresponding forward edge.
    for callee_name in graph.functions() {
        for caller_name in graph.callers_of(callee_name) {
            assert!(
                graph.callees_of(*caller_name).contains(&callee_name),
                "reverse edge {callee_name:?}←{caller_name:?} has no forward entry"
            );
        }
    }
}

#[test]
fn multiple_calls_to_same_function() {
    // A calls B twice in different instructions — should only appear once in callees.
    let (a, b_name) = (n(1), n(2));
    let func = make_func_named(
        a,
        vec![owned_param(0, Idx::INT)],
        Idx::INT,
        vec![ArcBlock {
            id: b(0),
            params: vec![],
            body: vec![
                ArcInstr::Apply {
                    dst: v(1),
                    ty: Idx::INT,
                    func: b_name,
                    args: vec![v(0)],
                    arg_ownership: vec![ArgOwnership::Owned],
                },
                ArcInstr::Apply {
                    dst: v(2),
                    ty: Idx::INT,
                    func: b_name,
                    args: vec![v(1)],
                    arg_ownership: vec![ArgOwnership::Owned],
                },
            ],
            terminator: ArcTerminator::Return { value: v(2) },
        }],
        vec![Idx::INT; 3],
    );
    let funcs = vec![func, leaf_func(b_name, 1)];
    let graph = CallGraph::build(&funcs);

    // B appears exactly once in A's callee set (FxHashSet deduplicates).
    assert_eq!(graph.callees_of(a).len(), 1);
    assert!(graph.callees_of(a).contains(&b_name));
    // A appears exactly once in B's caller set.
    assert_eq!(graph.callers_of(b_name).len(), 1);
}
