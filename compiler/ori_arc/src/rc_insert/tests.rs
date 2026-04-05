//! Tests for argument ownership annotation on indirect calls.
//!
//! Covers `annotate_arg_ownership()` behavior for `ApplyIndirect` and
//! `InvokeIndirect` instructions, including closure resolution through
//! SSA def maps, block params, and opaque fallback.

use ori_ir::{Name, StringInterner};
use ori_types::{Idx, Pool};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::ir::{
    ArcBlock, ArcBlockId, ArcFunction, ArcInstr, ArcParam, ArcTerminator, ArcVarId, ArgOwnership,
};
use crate::ownership::{AnnotatedParam, AnnotatedSig, Ownership};
use crate::BuiltinOwnershipSets;

/// Helper to create an empty `BuiltinOwnershipSets`.
fn empty_builtins() -> BuiltinOwnershipSets {
    BuiltinOwnershipSets {
        borrowing: FxHashSet::default(),
        consuming_receiver: FxHashSet::default(),
        consuming_second_arg: FxHashSet::default(),
        consuming_receiver_only: FxHashSet::default(),
        protocol: FxHashMap::default(),
    }
}

/// Helper to create an `AnnotatedSig` with the given ownership per param.
fn make_sig(ownerships: &[Ownership]) -> AnnotatedSig {
    AnnotatedSig {
        params: ownerships
            .iter()
            .enumerate()
            .map(|(i, &o)| AnnotatedParam {
                #[expect(
                    clippy::cast_possible_truncation,
                    reason = "test helper with < 256 params"
                )]
                name: Name::from_raw(i as u32),
                ty: Idx::NONE,
                ownership: o,
            })
            .collect(),
        return_type: Idx::NONE,
    }
}

/// Helper: minimal `ArcFunction` with given blocks, var count, and params.
fn make_func(
    name: Name,
    params: Vec<ArcParam>,
    blocks: Vec<ArcBlock>,
    var_count: usize,
) -> ArcFunction {
    ArcFunction {
        name,
        params,
        return_type: Idx::NONE,
        blocks,
        entry: ArcBlockId::new(0),
        var_types: vec![Idx::INT; var_count],
        var_reprs: vec![],
        spans: vec![],
        is_fbip: false,
        num_captures: 0,
        cow_annotations: crate::uniqueness::CowAnnotations::default(),
        drop_hints: crate::uniqueness::DropHints::default(),
        tail_calls: vec![],
    }
}

// ──────────────────────────────────────────────────────────────────────
// Resolver-level tests
// ──────────────────────────────────────────────────────────────────────

#[test]
fn test_annotate_apply_indirect_from_partial_apply() {
    let interner = StringInterner::new();
    let pool = Pool::new();
    let builtins = empty_builtins();

    let target_name = interner.intern("my_func");
    let func_name = interner.intern("caller");

    // v0 = param, v1 = PartialApply(my_func, [v0]), v2 = ApplyIndirect(v1, [v3])
    // my_func sig: [Owned, Owned] (capture + user arg)
    let blocks = vec![ArcBlock {
        id: ArcBlockId::new(0),
        params: vec![],
        body: vec![
            ArcInstr::PartialApply {
                dst: ArcVarId::new(1),
                ty: Idx::NONE,
                func: target_name,
                args: vec![ArcVarId::new(0)], // 1 capture
            },
            ArcInstr::ApplyIndirect {
                dst: ArcVarId::new(4),
                ty: Idx::INT,
                closure: ArcVarId::new(1),
                args: vec![ArcVarId::new(3)], // 1 user arg
                arg_ownership: vec![],
            },
        ],
        terminator: ArcTerminator::Return {
            value: ArcVarId::new(4),
        },
    }];

    let mut func = make_func(func_name, vec![], blocks, 5);

    // Sig for my_func: 2 params (1 capture + 1 user), both Owned.
    let mut sigs = FxHashMap::default();
    sigs.insert(target_name, make_sig(&[Ownership::Owned, Ownership::Owned]));

    super::annotate_arg_ownership(&mut func, &sigs, &interner, &builtins, &pool);

    // ApplyIndirect should have user-arg ownership = [Owned] (from sig param[1]).
    if let ArcInstr::ApplyIndirect { arg_ownership, .. } = &func.blocks[0].body[1] {
        assert_eq!(arg_ownership, &[ArgOwnership::Owned]);
    } else {
        panic!("expected ApplyIndirect");
    }
}

#[test]
fn test_annotate_apply_indirect_opaque_closure() {
    let interner = StringInterner::new();
    let pool = Pool::new();
    let builtins = empty_builtins();
    let func_name = interner.intern("caller");

    // v0 = function param (opaque closure), v1 = ApplyIndirect(v0, [v2])
    let blocks = vec![ArcBlock {
        id: ArcBlockId::new(0),
        params: vec![],
        body: vec![ArcInstr::ApplyIndirect {
            dst: ArcVarId::new(3),
            ty: Idx::INT,
            closure: ArcVarId::new(0), // param — not traceable
            args: vec![ArcVarId::new(2)],
            arg_ownership: vec![],
        }],
        terminator: ArcTerminator::Return {
            value: ArcVarId::new(3),
        },
    }];

    let params = vec![ArcParam {
        var: ArcVarId::new(0),
        ty: Idx::NONE,
        ownership: Ownership::Borrowed,
    }];
    let mut func = make_func(func_name, params, blocks, 4);
    let sigs = FxHashMap::default();

    super::annotate_arg_ownership(&mut func, &sigs, &interner, &builtins, &pool);

    if let ArcInstr::ApplyIndirect { arg_ownership, .. } = &func.blocks[0].body[0] {
        assert_eq!(
            arg_ownership,
            &[ArgOwnership::Borrowed],
            "opaque closure must default to all-Borrowed"
        );
    } else {
        panic!("expected ApplyIndirect");
    }
}

#[test]
fn test_annotate_invoke_indirect() {
    let interner = StringInterner::new();
    let pool = Pool::new();
    let builtins = empty_builtins();

    let target_name = interner.intern("target_fn");
    let func_name = interner.intern("caller");

    // v0 = PartialApply(target_fn, []), v1 = InvokeIndirect(v0, [v2])
    let blocks = vec![ArcBlock {
        id: ArcBlockId::new(0),
        params: vec![],
        body: vec![ArcInstr::PartialApply {
            dst: ArcVarId::new(0),
            ty: Idx::NONE,
            func: target_name,
            args: vec![], // 0 captures
        }],
        terminator: ArcTerminator::InvokeIndirect {
            dst: ArcVarId::new(3),
            ty: Idx::INT,
            closure: ArcVarId::new(0),
            args: vec![ArcVarId::new(2)],
            arg_ownership: vec![],
            normal: ArcBlockId::new(1),
            unwind: ArcBlockId::new(2),
        },
    }];

    let mut func = make_func(func_name, vec![], blocks, 4);
    let mut sigs = FxHashMap::default();
    sigs.insert(target_name, make_sig(&[Ownership::Owned]));

    super::annotate_arg_ownership(&mut func, &sigs, &interner, &builtins, &pool);

    if let ArcTerminator::InvokeIndirect { arg_ownership, .. } = &func.blocks[0].terminator {
        assert_eq!(arg_ownership, &[ArgOwnership::Owned]);
    } else {
        panic!("expected InvokeIndirect");
    }
}

#[test]
fn test_annotate_apply_indirect_with_captures_offset() {
    let interner = StringInterner::new();
    let pool = Pool::new();
    let builtins = empty_builtins();

    let target_name = interner.intern("target");
    let func_name = interner.intern("caller");

    // PartialApply with 2 captures, sig has 4 params [Own, Borrow, Own, Borrow]
    // User args see params[2..4] = [Own, Borrow]
    let blocks = vec![ArcBlock {
        id: ArcBlockId::new(0),
        params: vec![],
        body: vec![
            ArcInstr::PartialApply {
                dst: ArcVarId::new(5),
                ty: Idx::NONE,
                func: target_name,
                args: vec![ArcVarId::new(0), ArcVarId::new(1)], // 2 captures
            },
            ArcInstr::ApplyIndirect {
                dst: ArcVarId::new(6),
                ty: Idx::INT,
                closure: ArcVarId::new(5),
                args: vec![ArcVarId::new(3), ArcVarId::new(4)], // 2 user args
                arg_ownership: vec![],
            },
        ],
        terminator: ArcTerminator::Return {
            value: ArcVarId::new(6),
        },
    }];

    let mut func = make_func(func_name, vec![], blocks, 7);
    let mut sigs = FxHashMap::default();
    sigs.insert(
        target_name,
        make_sig(&[
            Ownership::Owned,
            Ownership::Borrowed,
            Ownership::Owned,
            Ownership::Borrowed,
        ]),
    );

    super::annotate_arg_ownership(&mut func, &sigs, &interner, &builtins, &pool);

    if let ArcInstr::ApplyIndirect { arg_ownership, .. } = &func.blocks[0].body[1] {
        assert_eq!(
            arg_ownership,
            &[ArgOwnership::Owned, ArgOwnership::Borrowed],
            "user args should get ownership from sig params [2..4], not [0..2]"
        );
    } else {
        panic!("expected ApplyIndirect");
    }
}

#[test]
fn test_annotate_apply_indirect_zero_capture_function_ref() {
    let interner = StringInterner::new();
    let pool = Pool::new();
    let builtins = empty_builtins();

    let target_name = interner.intern("func_ref");
    let func_name = interner.intern("caller");

    // Zero-capture PartialApply = function reference.
    // Should annotate exactly like a direct call.
    let blocks = vec![ArcBlock {
        id: ArcBlockId::new(0),
        params: vec![],
        body: vec![
            ArcInstr::PartialApply {
                dst: ArcVarId::new(0),
                ty: Idx::NONE,
                func: target_name,
                args: vec![], // 0 captures
            },
            ArcInstr::ApplyIndirect {
                dst: ArcVarId::new(3),
                ty: Idx::INT,
                closure: ArcVarId::new(0),
                args: vec![ArcVarId::new(1), ArcVarId::new(2)],
                arg_ownership: vec![],
            },
        ],
        terminator: ArcTerminator::Return {
            value: ArcVarId::new(3),
        },
    }];

    let mut func = make_func(func_name, vec![], blocks, 4);
    let mut sigs = FxHashMap::default();
    sigs.insert(
        target_name,
        make_sig(&[Ownership::Owned, Ownership::Borrowed]),
    );

    super::annotate_arg_ownership(&mut func, &sigs, &interner, &builtins, &pool);

    if let ArcInstr::ApplyIndirect { arg_ownership, .. } = &func.blocks[0].body[1] {
        assert_eq!(
            arg_ownership,
            &[ArgOwnership::Owned, ArgOwnership::Borrowed]
        );
    } else {
        panic!("expected ApplyIndirect");
    }
}

#[test]
fn test_annotate_apply_indirect_alias_across_blocks() {
    let interner = StringInterner::new();
    let pool = Pool::new();
    let builtins = empty_builtins();

    let target_name = interner.intern("target");
    let func_name = interner.intern("caller");

    // Block 0: v1 = PartialApply(target, [v0]), v2 = Let(Var(v1)) alias
    //          Jump to block 1 with [v2]
    // Block 1: params=[v3], ApplyIndirect(v3, [v4])
    // v3 traces through block param → v2 → alias → v1 → PartialApply
    let blocks = vec![
        ArcBlock {
            id: ArcBlockId::new(0),
            params: vec![],
            body: vec![
                ArcInstr::PartialApply {
                    dst: ArcVarId::new(1),
                    ty: Idx::NONE,
                    func: target_name,
                    args: vec![ArcVarId::new(0)],
                },
                ArcInstr::Let {
                    dst: ArcVarId::new(2),
                    ty: Idx::NONE,
                    value: crate::ir::ArcValue::Var(ArcVarId::new(1)),
                },
            ],
            terminator: ArcTerminator::Jump {
                target: ArcBlockId::new(1),
                args: vec![ArcVarId::new(2)],
            },
        },
        ArcBlock {
            id: ArcBlockId::new(1),
            params: vec![(ArcVarId::new(3), Idx::NONE)],
            body: vec![ArcInstr::ApplyIndirect {
                dst: ArcVarId::new(5),
                ty: Idx::INT,
                closure: ArcVarId::new(3),
                args: vec![ArcVarId::new(4)],
                arg_ownership: vec![],
            }],
            terminator: ArcTerminator::Return {
                value: ArcVarId::new(5),
            },
        },
    ];

    let mut func = make_func(func_name, vec![], blocks, 6);
    let mut sigs = FxHashMap::default();
    sigs.insert(target_name, make_sig(&[Ownership::Owned, Ownership::Owned]));

    super::annotate_arg_ownership(&mut func, &sigs, &interner, &builtins, &pool);

    if let ArcInstr::ApplyIndirect { arg_ownership, .. } = &func.blocks[1].body[0] {
        assert_eq!(
            arg_ownership,
            &[ArgOwnership::Owned],
            "should resolve through block param + alias to PartialApply"
        );
    } else {
        panic!("expected ApplyIndirect");
    }
}

#[test]
fn test_annotate_apply_indirect_merge_conflict_defaults_borrowed() {
    let interner = StringInterner::new();
    let pool = Pool::new();
    let builtins = empty_builtins();

    let target_a = interner.intern("func_a");
    let target_b = interner.intern("func_b");
    let func_name = interner.intern("caller");

    // Block 0: v1 = PartialApply(func_a, []), jump to block 2 with [v1]
    // Block 1: v2 = PartialApply(func_b, []), jump to block 2 with [v2]
    // Block 2: params=[v3], ApplyIndirect(v3, [v4])
    // Two different closures merge → should fall back to all-Borrowed
    let blocks = vec![
        ArcBlock {
            id: ArcBlockId::new(0),
            params: vec![],
            body: vec![ArcInstr::PartialApply {
                dst: ArcVarId::new(1),
                ty: Idx::NONE,
                func: target_a,
                args: vec![],
            }],
            terminator: ArcTerminator::Jump {
                target: ArcBlockId::new(2),
                args: vec![ArcVarId::new(1)],
            },
        },
        ArcBlock {
            id: ArcBlockId::new(1),
            params: vec![],
            body: vec![ArcInstr::PartialApply {
                dst: ArcVarId::new(2),
                ty: Idx::NONE,
                func: target_b,
                args: vec![],
            }],
            terminator: ArcTerminator::Jump {
                target: ArcBlockId::new(2),
                args: vec![ArcVarId::new(2)],
            },
        },
        ArcBlock {
            id: ArcBlockId::new(2),
            params: vec![(ArcVarId::new(3), Idx::NONE)],
            body: vec![ArcInstr::ApplyIndirect {
                dst: ArcVarId::new(5),
                ty: Idx::INT,
                closure: ArcVarId::new(3),
                args: vec![ArcVarId::new(4)],
                arg_ownership: vec![],
            }],
            terminator: ArcTerminator::Return {
                value: ArcVarId::new(5),
            },
        },
    ];

    let mut func = make_func(func_name, vec![], blocks, 6);
    let mut sigs = FxHashMap::default();
    sigs.insert(target_a, make_sig(&[Ownership::Owned]));
    sigs.insert(target_b, make_sig(&[Ownership::Borrowed]));

    super::annotate_arg_ownership(&mut func, &sigs, &interner, &builtins, &pool);

    if let ArcInstr::ApplyIndirect { arg_ownership, .. } = &func.blocks[2].body[0] {
        assert_eq!(
            arg_ownership,
            &[ArgOwnership::Borrowed],
            "conflicting closures must fall back to all-Borrowed"
        );
    } else {
        panic!("expected ApplyIndirect");
    }
}

#[test]
fn test_annotate_apply_indirect_loop_carried_block_param() {
    let interner = StringInterner::new();
    let pool = Pool::new();
    let builtins = empty_builtins();

    let target_name = interner.intern("target");
    let func_name = interner.intern("caller");

    // Block 0: v1 = PartialApply(target, [v0]), jump to block 1 with [v1]
    // Block 1: params=[v2], ApplyIndirect(v2, [v3]), jump to block 1 with [v2]
    // The back-edge creates a cycle: v2 → block param → v2
    // Must terminate, not infinite recurse.
    let blocks = vec![
        ArcBlock {
            id: ArcBlockId::new(0),
            params: vec![],
            body: vec![ArcInstr::PartialApply {
                dst: ArcVarId::new(1),
                ty: Idx::NONE,
                func: target_name,
                args: vec![ArcVarId::new(0)],
            }],
            terminator: ArcTerminator::Jump {
                target: ArcBlockId::new(1),
                args: vec![ArcVarId::new(1)],
            },
        },
        ArcBlock {
            id: ArcBlockId::new(1),
            params: vec![(ArcVarId::new(2), Idx::NONE)],
            body: vec![ArcInstr::ApplyIndirect {
                dst: ArcVarId::new(4),
                ty: Idx::INT,
                closure: ArcVarId::new(2),
                args: vec![ArcVarId::new(3)],
                arg_ownership: vec![],
            }],
            terminator: ArcTerminator::Jump {
                target: ArcBlockId::new(1),
                args: vec![ArcVarId::new(2)], // back-edge: same closure
            },
        },
    ];

    let mut func = make_func(func_name, vec![], blocks, 5);
    let mut sigs = FxHashMap::default();
    sigs.insert(target_name, make_sig(&[Ownership::Owned, Ownership::Owned]));

    super::annotate_arg_ownership(&mut func, &sigs, &interner, &builtins, &pool);

    // Must not hang. The cycle should resolve because both predecessors
    // point to the same PartialApply target.
    if let ArcInstr::ApplyIndirect { arg_ownership, .. } = &func.blocks[1].body[0] {
        assert_eq!(arg_ownership, &[ArgOwnership::Owned]);
    } else {
        panic!("expected ApplyIndirect");
    }
}

#[test]
fn test_annotate_apply_indirect_zero_user_args() {
    let interner = StringInterner::new();
    let pool = Pool::new();
    let builtins = empty_builtins();

    let target_name = interner.intern("thunk");
    let func_name = interner.intern("caller");

    // Thunk: PartialApply with captures, then ApplyIndirect with 0 user args
    let blocks = vec![ArcBlock {
        id: ArcBlockId::new(0),
        params: vec![],
        body: vec![
            ArcInstr::PartialApply {
                dst: ArcVarId::new(1),
                ty: Idx::NONE,
                func: target_name,
                args: vec![ArcVarId::new(0)],
            },
            ArcInstr::ApplyIndirect {
                dst: ArcVarId::new(2),
                ty: Idx::INT,
                closure: ArcVarId::new(1),
                args: vec![], // zero user args
                arg_ownership: vec![],
            },
        ],
        terminator: ArcTerminator::Return {
            value: ArcVarId::new(2),
        },
    }];

    let mut func = make_func(func_name, vec![], blocks, 3);
    let sigs = FxHashMap::default();

    super::annotate_arg_ownership(&mut func, &sigs, &interner, &builtins, &pool);

    if let ArcInstr::ApplyIndirect { arg_ownership, .. } = &func.blocks[0].body[1] {
        assert!(arg_ownership.is_empty(), "zero user args → empty ownership");
    } else {
        panic!("expected ApplyIndirect");
    }
}

#[test]
fn test_annotate_apply_indirect_opaque_not_owned() {
    // Negative pin: opaque closure must NEVER produce Owned for any arg.
    let interner = StringInterner::new();
    let pool = Pool::new();
    let builtins = empty_builtins();
    let func_name = interner.intern("caller");

    let blocks = vec![ArcBlock {
        id: ArcBlockId::new(0),
        params: vec![],
        body: vec![ArcInstr::ApplyIndirect {
            dst: ArcVarId::new(4),
            ty: Idx::INT,
            closure: ArcVarId::new(0), // opaque
            args: vec![ArcVarId::new(1), ArcVarId::new(2), ArcVarId::new(3)],
            arg_ownership: vec![],
        }],
        terminator: ArcTerminator::Return {
            value: ArcVarId::new(4),
        },
    }];

    let mut func = make_func(func_name, vec![], blocks, 5);
    let sigs = FxHashMap::default();

    super::annotate_arg_ownership(&mut func, &sigs, &interner, &builtins, &pool);

    if let ArcInstr::ApplyIndirect { arg_ownership, .. } = &func.blocks[0].body[0] {
        for (i, o) in arg_ownership.iter().enumerate() {
            assert_eq!(
                *o,
                ArgOwnership::Borrowed,
                "arg[{i}] must be Borrowed for opaque closure"
            );
        }
    } else {
        panic!("expected ApplyIndirect");
    }
}

#[test]
fn test_annotate_apply_indirect_builtin_partial_apply() {
    let interner = StringInterner::new();
    let mut pool = Pool::new();

    let push_name = interner.intern("push");
    let func_name = interner.intern("caller");

    // Set up builtins: push is consuming_receiver for List type
    let mut builtins = empty_builtins();
    builtins.consuming_receiver.insert(push_name);

    // PartialApply(push, [list_var]) — captures the list (receiver)
    // ApplyIndirect(closure, [element]) — 1 user arg (the element to push)
    //
    // push sig: [Owned, Borrowed] (receiver consumed, element borrowed)
    // After capture offset: user args get [Borrowed] (param[1] onwards)
    //
    // But consuming_receiver override makes param[0] Owned for List.
    // Since param[0] is a capture (not a user arg), the user args
    // still see Borrowed from sig params[1].
    let blocks = vec![ArcBlock {
        id: ArcBlockId::new(0),
        params: vec![],
        body: vec![
            ArcInstr::PartialApply {
                dst: ArcVarId::new(1),
                ty: Idx::NONE,
                func: push_name,
                args: vec![ArcVarId::new(0)], // capture the list
            },
            ArcInstr::ApplyIndirect {
                dst: ArcVarId::new(3),
                ty: Idx::INT,
                closure: ArcVarId::new(1),
                args: vec![ArcVarId::new(2)], // 1 user arg: element
                arg_ownership: vec![],
            },
        ],
        terminator: ArcTerminator::Return {
            value: ArcVarId::new(3),
        },
    }];

    // v0 is the list — type must be List for consuming override to fire
    let mut var_types = vec![Idx::INT; 4];
    // We need v0 to be List type. Use a pool to create a List<int> type.
    let list_int = pool.list(Idx::INT);
    var_types[0] = list_int;

    let mut func = make_func(func_name, vec![], blocks, 4);
    func.var_types = var_types;

    let mut sigs = FxHashMap::default();
    // push sig: receiver(Owned) + element(Borrowed)
    sigs.insert(
        push_name,
        make_sig(&[Ownership::Owned, Ownership::Borrowed]),
    );

    super::annotate_arg_ownership(&mut func, &sigs, &interner, &builtins, &pool);

    if let ArcInstr::ApplyIndirect { arg_ownership, .. } = &func.blocks[0].body[1] {
        // The user arg (element) should be Borrowed — the Owned receiver is a capture
        assert_eq!(
            arg_ownership,
            &[ArgOwnership::Borrowed],
            "user arg should be Borrowed (Owned receiver is captured)"
        );
    } else {
        panic!("expected ApplyIndirect");
    }
}

/// Regression: TPR-02-004 — same-name builtins with different-typed captures
/// must fall back (type-qualified overrides can diverge).
#[test]
fn test_annotate_apply_indirect_different_capture_types_defaults_borrowed() {
    let interner = StringInterner::new();
    let mut pool = Pool::new();

    let target = interner.intern("concat");
    let func_name = interner.intern("caller");

    // Register concat as consuming_receiver + consuming_second_arg
    // so apply_consuming_overrides actually fires for List but not str.
    let mut builtins = empty_builtins();
    builtins.consuming_receiver.insert(target);
    builtins.consuming_second_arg.insert(target);

    // v0: List<int>, v1: str — different types for the capture position.
    // `apply_consuming_overrides` fires for List (marks receiver+second Owned)
    // but not str (no override), so effective ownership diverges → must fall back.
    let list_int = pool.list(Idx::INT);

    let blocks = vec![
        ArcBlock {
            id: ArcBlockId::new(0),
            params: vec![],
            body: vec![ArcInstr::PartialApply {
                dst: ArcVarId::new(2),
                ty: Idx::NONE,
                func: target,
                args: vec![ArcVarId::new(0)], // captures list
            }],
            terminator: ArcTerminator::Jump {
                target: ArcBlockId::new(2),
                args: vec![ArcVarId::new(2)],
            },
        },
        ArcBlock {
            id: ArcBlockId::new(1),
            params: vec![],
            body: vec![ArcInstr::PartialApply {
                dst: ArcVarId::new(3),
                ty: Idx::NONE,
                func: target,
                args: vec![ArcVarId::new(1)], // captures str
            }],
            terminator: ArcTerminator::Jump {
                target: ArcBlockId::new(2),
                args: vec![ArcVarId::new(3)],
            },
        },
        ArcBlock {
            id: ArcBlockId::new(2),
            params: vec![(ArcVarId::new(4), Idx::NONE)],
            body: vec![ArcInstr::ApplyIndirect {
                dst: ArcVarId::new(6),
                ty: Idx::INT,
                closure: ArcVarId::new(4),
                args: vec![ArcVarId::new(5)],
                arg_ownership: vec![],
            }],
            terminator: ArcTerminator::Return {
                value: ArcVarId::new(6),
            },
        },
    ];

    let mut func = make_func(func_name, vec![], blocks, 7);
    // v0=List<int>, v1=str — different types
    func.var_types[0] = list_int;
    func.var_types[1] = Idx::STR;

    let mut sigs = FxHashMap::default();
    sigs.insert(target, make_sig(&[Ownership::Owned, Ownership::Owned]));

    super::annotate_arg_ownership(&mut func, &sigs, &interner, &builtins, &pool);

    if let ArcInstr::ApplyIndirect { arg_ownership, .. } = &func.blocks[2].body[0] {
        assert_eq!(
            arg_ownership,
            &[ArgOwnership::Borrowed],
            "different capture types must fall back to Borrowed (TPR-02-004)"
        );
    } else {
        panic!("expected ApplyIndirect");
    }
}

/// Regression: TPR-02-008 — same-name builtins with same-typed captures
/// must merge successfully (type comparison allows it).
#[test]
fn test_annotate_apply_indirect_same_capture_types_merges() {
    let interner = StringInterner::new();
    let pool = Pool::new();
    let builtins = empty_builtins();

    let target = interner.intern("target");
    let func_name = interner.intern("caller");

    // Both capture vars are int (same type) → merge should succeed.
    let blocks = vec![
        ArcBlock {
            id: ArcBlockId::new(0),
            params: vec![],
            body: vec![ArcInstr::PartialApply {
                dst: ArcVarId::new(2),
                ty: Idx::NONE,
                func: target,
                args: vec![ArcVarId::new(0)],
            }],
            terminator: ArcTerminator::Jump {
                target: ArcBlockId::new(2),
                args: vec![ArcVarId::new(2)],
            },
        },
        ArcBlock {
            id: ArcBlockId::new(1),
            params: vec![],
            body: vec![ArcInstr::PartialApply {
                dst: ArcVarId::new(3),
                ty: Idx::NONE,
                func: target,
                args: vec![ArcVarId::new(1)], // different var, same type
            }],
            terminator: ArcTerminator::Jump {
                target: ArcBlockId::new(2),
                args: vec![ArcVarId::new(3)],
            },
        },
        ArcBlock {
            id: ArcBlockId::new(2),
            params: vec![(ArcVarId::new(4), Idx::NONE)],
            body: vec![ArcInstr::ApplyIndirect {
                dst: ArcVarId::new(6),
                ty: Idx::INT,
                closure: ArcVarId::new(4),
                args: vec![ArcVarId::new(5)],
                arg_ownership: vec![],
            }],
            terminator: ArcTerminator::Return {
                value: ArcVarId::new(6),
            },
        },
    ];

    let mut func = make_func(func_name, vec![], blocks, 7);
    let mut sigs = FxHashMap::default();
    sigs.insert(target, make_sig(&[Ownership::Owned, Ownership::Owned]));

    super::annotate_arg_ownership(&mut func, &sigs, &interner, &builtins, &pool);

    if let ArcInstr::ApplyIndirect { arg_ownership, .. } = &func.blocks[2].body[0] {
        assert_eq!(
            arg_ownership,
            &[ArgOwnership::Owned],
            "same capture types must merge (TPR-02-008)"
        );
    } else {
        panic!("expected ApplyIndirect");
    }
}

/// Regression: TPR-02-005 — diamond CFG where two predecessors reach the
/// same `PartialApply` through different alias paths must still resolve
/// (not fall back to opaque due to shared visited set).
#[test]
fn test_annotate_apply_indirect_diamond_cfg_same_origin() {
    let interner = StringInterner::new();
    let pool = Pool::new();
    let builtins = empty_builtins();

    let target = interner.intern("target");
    let func_name = interner.intern("caller");

    // Block 0: v1 = PartialApply(target, [v0])
    //          v2 = Let(Var(v1))   — alias path A
    //          v3 = Let(Var(v1))   — alias path B
    //          Branch to block 1 (with v2) or block 2 (with v3)
    // Block 1: params=[], jump to block 3 with [v2]
    // Block 2: params=[], jump to block 3 with [v3]
    // Block 3: params=[v4], ApplyIndirect(v4, [v5])
    // Both paths converge on the same PartialApply → must resolve.
    let blocks = vec![
        ArcBlock {
            id: ArcBlockId::new(0),
            params: vec![],
            body: vec![
                ArcInstr::PartialApply {
                    dst: ArcVarId::new(1),
                    ty: Idx::NONE,
                    func: target,
                    args: vec![ArcVarId::new(0)],
                },
                ArcInstr::Let {
                    dst: ArcVarId::new(2),
                    ty: Idx::NONE,
                    value: crate::ir::ArcValue::Var(ArcVarId::new(1)),
                },
                ArcInstr::Let {
                    dst: ArcVarId::new(3),
                    ty: Idx::NONE,
                    value: crate::ir::ArcValue::Var(ArcVarId::new(1)),
                },
            ],
            terminator: ArcTerminator::Branch {
                cond: ArcVarId::new(0),
                then_block: ArcBlockId::new(1),
                else_block: ArcBlockId::new(2),
            },
        },
        ArcBlock {
            id: ArcBlockId::new(1),
            params: vec![],
            body: vec![],
            terminator: ArcTerminator::Jump {
                target: ArcBlockId::new(3),
                args: vec![ArcVarId::new(2)], // alias path A
            },
        },
        ArcBlock {
            id: ArcBlockId::new(2),
            params: vec![],
            body: vec![],
            terminator: ArcTerminator::Jump {
                target: ArcBlockId::new(3),
                args: vec![ArcVarId::new(3)], // alias path B
            },
        },
        ArcBlock {
            id: ArcBlockId::new(3),
            params: vec![(ArcVarId::new(4), Idx::NONE)],
            body: vec![ArcInstr::ApplyIndirect {
                dst: ArcVarId::new(6),
                ty: Idx::INT,
                closure: ArcVarId::new(4),
                args: vec![ArcVarId::new(5)],
                arg_ownership: vec![],
            }],
            terminator: ArcTerminator::Return {
                value: ArcVarId::new(6),
            },
        },
    ];

    let mut func = make_func(func_name, vec![], blocks, 7);
    let mut sigs = FxHashMap::default();
    sigs.insert(target, make_sig(&[Ownership::Owned, Ownership::Owned]));

    super::annotate_arg_ownership(&mut func, &sigs, &interner, &builtins, &pool);

    if let ArcInstr::ApplyIndirect { arg_ownership, .. } = &func.blocks[3].body[0] {
        assert_eq!(
            arg_ownership,
            &[ArgOwnership::Owned],
            "diamond CFG must resolve to same origin (TPR-02-005)"
        );
    } else {
        panic!("expected ApplyIndirect");
    }
}
