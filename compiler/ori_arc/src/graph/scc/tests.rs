#![expect(clippy::unwrap_used, reason = "Tests use unwrap for brevity")]

use ori_ir::Name;
use ori_types::Idx;

use crate::graph::call_graph::CallGraph;
use crate::ir::{ArcBlock, ArcInstr, ArcTerminator, ArgOwnership};
use crate::test_helpers::{b, make_func_named, owned_param, v};

use super::*;

/// Shorthand for `Name::from_raw(n)`.
fn n(raw: u32) -> Name {
    Name::from_raw(raw)
}

/// Build a leaf function (no calls).
fn leaf(name: Name) -> crate::ir::ArcFunction {
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
        vec![Idx::INT],
    )
}

/// Build a function that calls `callee` via Apply.
fn calls(name: Name, callee: Name) -> crate::ir::ArcFunction {
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
        vec![Idx::INT; 2],
    )
}

/// Build a function that calls multiple callees.
fn calls_many(name: Name, callees: &[Name]) -> crate::ir::ArcFunction {
    let mut body = Vec::new();
    for (i, callee) in callees.iter().enumerate() {
        #[expect(clippy::cast_possible_truncation, reason = "test code, small values")]
        let dst_raw = (i as u32) + 1;
        let prev = if i == 0 { v(0) } else { v(dst_raw - 1) };
        body.push(ArcInstr::Apply {
            dst: v(dst_raw),
            ty: Idx::INT,
            func: *callee,
            args: vec![prev],
            arg_ownership: vec![ArgOwnership::Owned],
        });
    }
    #[expect(clippy::cast_possible_truncation, reason = "test code, small values")]
    let last_var = v(callees.len() as u32);
    make_func_named(
        name,
        vec![owned_param(0, Idx::INT)],
        Idx::INT,
        vec![ArcBlock {
            id: b(0),
            params: vec![],
            body,
            terminator: ArcTerminator::Return { value: last_var },
        }],
        vec![Idx::INT; callees.len() + 1],
    )
}

/// Find which SCC contains a given name.
fn find_scc(sccs: &[Scc], name: Name) -> Option<&Scc> {
    sccs.iter().find(|scc| scc.members.contains(&name))
}

// --- Tests ---

#[test]
fn no_functions() {
    let graph = CallGraph::build(&[]);
    let sccs = compute_sccs(&graph);
    assert!(sccs.is_empty());
}

#[test]
fn single_leaf() {
    let a = n(1);
    let graph = CallGraph::build(&[leaf(a)]);
    let sccs = compute_sccs(&graph);

    assert_eq!(sccs.len(), 1);
    assert_eq!(sccs[0].members.len(), 1);
    assert!(sccs[0].members.contains(&a));
    assert!(!sccs[0].is_recursive(&graph));
}

#[test]
fn linear_chain() {
    // A → B → C: three SCCs of size 1, topological order [C, B, A]
    let (a, b_name, c) = (n(1), n(2), n(3));
    let funcs = vec![calls(a, b_name), calls(b_name, c), leaf(c)];
    let graph = CallGraph::build(&funcs);
    let sccs = compute_sccs(&graph);

    assert_eq!(sccs.len(), 3);

    // Each SCC has exactly 1 member.
    for scc in &sccs {
        assert_eq!(scc.members.len(), 1);
        assert!(!scc.is_recursive(&graph));
    }

    // In topological_order (forward), C comes before B, B before A.
    let topo = topological_order(&sccs);
    let c_pos = topo.iter().position(|s| s.members.contains(&c)).unwrap();
    let b_pos = topo
        .iter()
        .position(|s| s.members.contains(&b_name))
        .unwrap();
    let a_pos = topo.iter().position(|s| s.members.contains(&a)).unwrap();
    assert!(c_pos < b_pos, "C should come before B in topological order");
    assert!(b_pos < a_pos, "B should come before A in topological order");
}

#[test]
fn simple_cycle() {
    // A → B, B → A: one SCC of size 2
    let (a, b_name) = (n(1), n(2));
    let funcs = vec![calls(a, b_name), calls(b_name, a)];
    let graph = CallGraph::build(&funcs);
    let sccs = compute_sccs(&graph);

    assert_eq!(sccs.len(), 1);
    assert_eq!(sccs[0].members.len(), 2);
    assert!(sccs[0].members.contains(&a));
    assert!(sccs[0].members.contains(&b_name));
    assert!(sccs[0].is_recursive(&graph));
}

#[test]
fn diamond() {
    // A → B, A → C, B → D, C → D: four SCCs of size 1
    // Topological: D before B and C, B and C before A
    let (a, b_name, c, d) = (n(1), n(2), n(3), n(4));
    let funcs = vec![
        calls_many(a, &[b_name, c]),
        calls(b_name, d),
        calls(c, d),
        leaf(d),
    ];
    let graph = CallGraph::build(&funcs);
    let sccs = compute_sccs(&graph);

    assert_eq!(sccs.len(), 4);
    for scc in &sccs {
        assert_eq!(scc.members.len(), 1);
    }

    let topo = topological_order(&sccs);
    let d_pos = topo.iter().position(|s| s.members.contains(&d)).unwrap();
    let b_pos = topo
        .iter()
        .position(|s| s.members.contains(&b_name))
        .unwrap();
    let c_pos = topo.iter().position(|s| s.members.contains(&c)).unwrap();
    let a_pos = topo.iter().position(|s| s.members.contains(&a)).unwrap();

    assert!(d_pos < b_pos, "D should come before B");
    assert!(d_pos < c_pos, "D should come before C");
    assert!(b_pos < a_pos, "B should come before A");
    assert!(c_pos < a_pos, "C should come before A");
}

#[test]
fn self_recursive() {
    // A → A: one SCC of size 1, is_recursive = true
    let a = n(1);
    let func = calls(a, a);
    let graph = CallGraph::build(&[func]);
    let sccs = compute_sccs(&graph);

    assert_eq!(sccs.len(), 1);
    assert_eq!(sccs[0].members.len(), 1);
    assert!(sccs[0].is_recursive(&graph));
}

#[test]
fn mixed_recursive_and_linear() {
    // A → B, B → A (cycle), D → C (linear), both cycles call C
    // SCC({A,B}), SCC({C}), SCC({D})
    let (a, b_name, c, d) = (n(1), n(2), n(3), n(4));
    let funcs = vec![
        calls_many(a, &[b_name, c]),
        calls(b_name, a),
        leaf(c),
        calls(d, c),
    ];
    let graph = CallGraph::build(&funcs);
    let sccs = compute_sccs(&graph);

    assert_eq!(sccs.len(), 3);

    // A and B are in the same SCC.
    let ab_scc = find_scc(&sccs, a).unwrap();
    assert!(ab_scc.members.contains(&b_name));
    assert_eq!(ab_scc.members.len(), 2);
    assert!(ab_scc.is_recursive(&graph));

    // C is in its own SCC.
    let c_scc = find_scc(&sccs, c).unwrap();
    assert_eq!(c_scc.members.len(), 1);
    assert!(!c_scc.is_recursive(&graph));

    // D is in its own SCC.
    let d_scc = find_scc(&sccs, d).unwrap();
    assert_eq!(d_scc.members.len(), 1);
    assert!(!d_scc.is_recursive(&graph));

    // Topological: C comes before both {A,B} and D.
    let topo = topological_order(&sccs);
    let c_pos = topo.iter().position(|s| s.members.contains(&c)).unwrap();
    let ab_pos = topo.iter().position(|s| s.members.contains(&a)).unwrap();
    assert!(c_pos < ab_pos, "C should come before {{A,B}}");
}

#[test]
fn topological_order_callees_first() {
    // For every SCC, all cross-SCC callees appear earlier in topological order.
    let (a, b_name, c, d) = (n(1), n(2), n(3), n(4));
    let funcs = vec![
        calls_many(a, &[b_name, c]),
        calls(b_name, d),
        calls(c, d),
        leaf(d),
    ];
    let graph = CallGraph::build(&funcs);
    let sccs = compute_sccs(&graph);
    let topo = topological_order(&sccs);

    for (pos, scc) in topo.iter().enumerate() {
        for member in &scc.members {
            for callee in graph.callees_of(*member) {
                // Only check callees that are in the graph (not external).
                if !graph.contains(*callee) {
                    continue;
                }
                // If callee is in the same SCC, skip (internal edge).
                if scc.members.contains(callee) {
                    continue;
                }
                // Callee must be in an earlier SCC in topological order.
                let callee_pos = topo
                    .iter()
                    .position(|s| s.members.contains(callee))
                    .unwrap();
                assert!(
                    callee_pos < pos,
                    "callee {callee:?} (pos {callee_pos}) should come before caller {member:?} (pos {pos})"
                );
            }
        }
    }
}

#[test]
fn all_functions_covered() {
    // Every function in the graph appears in exactly one SCC.
    let (a, b_name, c, d) = (n(1), n(2), n(3), n(4));
    let funcs = vec![
        calls_many(a, &[b_name, c]),
        calls(b_name, a),
        calls(c, d),
        leaf(d),
    ];
    let graph = CallGraph::build(&funcs);
    let sccs = compute_sccs(&graph);

    let mut covered: FxHashSet<Name> = FxHashSet::default();
    for scc in &sccs {
        for member in &scc.members {
            assert!(
                covered.insert(*member),
                "function {member:?} appears in multiple SCCs"
            );
        }
    }

    for name in graph.functions() {
        assert!(covered.contains(&name), "function {name:?} not in any SCC");
    }
}
