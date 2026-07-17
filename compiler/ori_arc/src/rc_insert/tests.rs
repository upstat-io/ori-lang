//! Tests for argument ownership annotation on indirect calls.
//!
//! Covers `annotate_arg_ownership()` behavior for `ApplyIndirect` and
//! `InvokeIndirect` instructions, including closure resolution through
//! SSA def maps, block params, and opaque fallback.

use ori_ir::{Name, StringInterner};
use ori_types::{Idx, Pool};
use rustc_hash::FxHashMap;

use crate::ir::{ArcBlock, ArcBlockId, ArcInstr, ArcParam, ArcTerminator, ArcVarId, ArgOwnership};
use crate::ownership::{AnnotatedParam, AnnotatedSig, Ownership};
use crate::test_helpers::{make_apply, make_block, make_func_named};
use crate::BuiltinOwnershipSets;

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

fn annotate_insert_call(receiver_ty: Idx, pool: &Pool) -> Vec<ArgOwnership> {
    let interner = StringInterner::new();
    let insert = interner.intern("insert");
    let caller = interner.intern("caller");
    let builtins = BuiltinOwnershipSets::new(&interner);

    let blocks = vec![make_block(
        ArcBlockId::new(0),
        vec![make_apply(
            ArcVarId::new(3),
            receiver_ty,
            insert,
            vec![ArcVarId::new(0), ArcVarId::new(1), ArcVarId::new(2)],
            Vec::new(),
        )],
        ArcTerminator::Return {
            value: ArcVarId::new(3),
        },
    )];
    let mut func = make_func_named(
        caller,
        vec![],
        receiver_ty,
        blocks,
        vec![receiver_ty, Idx::INT, Idx::STR, receiver_ty],
    );
    let mut sigs = FxHashMap::default();
    sigs.insert(
        insert,
        make_sig(&[Ownership::Borrowed, Ownership::Borrowed, Ownership::Owned]),
    );

    super::annotate_arg_ownership(&mut func, &sigs, &interner, &builtins, pool);
    let ArcInstr::Apply { arg_ownership, .. } = &func.blocks[0].body[0] else {
        panic!("expected Apply");
    };
    arg_ownership.clone()
}

#[test]
fn map_insert_borrows_key_and_value_despite_list_contract_name_collision() {
    let mut pool = Pool::new();
    let map_ty = pool.map(Idx::INT, Idx::STR);

    assert_eq!(
        annotate_insert_call(map_ty, &pool),
        [
            ArgOwnership::Owned,
            ArgOwnership::Borrowed,
            ArgOwnership::Borrowed,
        ]
    );
}

#[test]
fn list_insert_still_consumes_value_after_map_insert_disambiguation() {
    let mut pool = Pool::new();
    let list_ty = pool.list(Idx::STR);

    assert_eq!(
        annotate_insert_call(list_ty, &pool),
        [
            ArgOwnership::Owned,
            ArgOwnership::Borrowed,
            ArgOwnership::Owned,
        ]
    );
}

// Residual indirect-call ABI tests

#[test]
fn indirect_call_is_borrowed_even_when_partial_apply_target_is_known() {
    let interner = StringInterner::new();
    let pool = Pool::new();
    let builtins = BuiltinOwnershipSets::empty();

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

    let mut func = make_func_named(func_name, vec![], Idx::NONE, blocks, vec![Idx::INT; 5]);

    // Sig for my_func: 2 params (1 capture + 1 user), both Owned.
    let mut sigs = FxHashMap::default();
    sigs.insert(target_name, make_sig(&[Ownership::Owned, Ownership::Owned]));

    super::annotate_arg_ownership(&mut func, &sigs, &interner, &builtins, &pool);

    // Target ownership is adapter-local; the residual caller ABI stays borrowed.
    if let ArcInstr::ApplyIndirect { arg_ownership, .. } = &func.blocks[0].body[1] {
        assert_eq!(arg_ownership, &[ArgOwnership::Borrowed]);
    } else {
        panic!("expected ApplyIndirect");
    }
}

#[test]
fn test_annotate_apply_indirect_opaque_closure() {
    let interner = StringInterner::new();
    let pool = Pool::new();
    let builtins = BuiltinOwnershipSets::empty();
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
    let mut func = make_func_named(func_name, params, Idx::NONE, blocks, vec![Idx::INT; 4]);
    let sigs = FxHashMap::default();

    super::annotate_arg_ownership(&mut func, &sigs, &interner, &builtins, &pool);

    if let ArcInstr::ApplyIndirect { arg_ownership, .. } = &func.blocks[0].body[0] {
        assert_eq!(
            arg_ownership,
            &[ArgOwnership::Borrowed],
            "opaque closure must use the same all-Borrowed residual ABI"
        );
    } else {
        panic!("expected ApplyIndirect");
    }
}

#[test]
fn invoke_indirect_uses_borrowed_residual_abi() {
    let interner = StringInterner::new();
    let pool = Pool::new();
    let builtins = BuiltinOwnershipSets::empty();

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

    let mut func = make_func_named(func_name, vec![], Idx::NONE, blocks, vec![Idx::INT; 4]);
    let mut sigs = FxHashMap::default();
    sigs.insert(target_name, make_sig(&[Ownership::Owned]));

    super::annotate_arg_ownership(&mut func, &sigs, &interner, &builtins, &pool);

    if let ArcTerminator::InvokeIndirect { arg_ownership, .. } = &func.blocks[0].terminator {
        assert_eq!(arg_ownership, &[ArgOwnership::Borrowed]);
    } else {
        panic!("expected InvokeIndirect");
    }
}

#[test]
fn indirect_call_does_not_project_target_ownership_through_capture_offset() {
    let interner = StringInterner::new();
    let pool = Pool::new();
    let builtins = BuiltinOwnershipSets::empty();

    let target_name = interner.intern("target");
    let func_name = interner.intern("caller");

    // PartialApply with 2 captures, sig has 4 params [Own, Borrow, Own, Borrow].
    // Target-specific ownership belongs to the adapter, not the caller.
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

    let mut func = make_func_named(func_name, vec![], Idx::NONE, blocks, vec![Idx::INT; 7]);
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
            &[ArgOwnership::Borrowed, ArgOwnership::Borrowed],
            "all explicit indirect-call arguments use the borrowed residual ABI"
        );
    } else {
        panic!("expected ApplyIndirect");
    }
}

#[test]
fn zero_capture_function_ref_still_uses_borrowed_residual_abi() {
    let interner = StringInterner::new();
    let pool = Pool::new();
    let builtins = BuiltinOwnershipSets::empty();

    let target_name = interner.intern("func_ref");
    let func_name = interner.intern("caller");

    // A zero-capture function reference remains an indirect call. Its direct
    // fast path is legal only when the shared adapter plan needs no retains.
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

    let mut func = make_func_named(func_name, vec![], Idx::NONE, blocks, vec![Idx::INT; 4]);
    let mut sigs = FxHashMap::default();
    sigs.insert(
        target_name,
        make_sig(&[Ownership::Owned, Ownership::Borrowed]),
    );

    super::annotate_arg_ownership(&mut func, &sigs, &interner, &builtins, &pool);

    if let ArcInstr::ApplyIndirect { arg_ownership, .. } = &func.blocks[0].body[1] {
        assert_eq!(arg_ownership, &[ArgOwnership::Borrowed; 2]);
    } else {
        panic!("expected ApplyIndirect");
    }
}

#[test]
fn indirect_call_alias_across_blocks_does_not_change_borrowed_abi() {
    let interner = StringInterner::new();
    let pool = Pool::new();
    let builtins = BuiltinOwnershipSets::empty();

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

    let mut func = make_func_named(func_name, vec![], Idx::NONE, blocks, vec![Idx::INT; 6]);
    let mut sigs = FxHashMap::default();
    sigs.insert(target_name, make_sig(&[Ownership::Owned, Ownership::Owned]));

    super::annotate_arg_ownership(&mut func, &sigs, &interner, &builtins, &pool);

    if let ArcInstr::ApplyIndirect { arg_ownership, .. } = &func.blocks[1].body[0] {
        assert_eq!(
            arg_ownership,
            &[ArgOwnership::Borrowed],
            "SSA provenance must not change the residual closure-call ABI"
        );
    } else {
        panic!("expected ApplyIndirect");
    }
}

#[test]
fn test_annotate_apply_indirect_merge_conflict_defaults_borrowed() {
    let interner = StringInterner::new();
    let pool = Pool::new();
    let builtins = BuiltinOwnershipSets::empty();

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

    let mut func = make_func_named(func_name, vec![], Idx::NONE, blocks, vec![Idx::INT; 6]);
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
fn loop_carried_indirect_call_uses_borrowed_abi_without_resolution() {
    let interner = StringInterner::new();
    let pool = Pool::new();
    let builtins = BuiltinOwnershipSets::empty();

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

    let mut func = make_func_named(func_name, vec![], Idx::NONE, blocks, vec![Idx::INT; 5]);
    let mut sigs = FxHashMap::default();
    sigs.insert(target_name, make_sig(&[Ownership::Owned, Ownership::Owned]));

    super::annotate_arg_ownership(&mut func, &sigs, &interner, &builtins, &pool);

    // No provenance walk is needed, so the cycle cannot affect ownership.
    if let ArcInstr::ApplyIndirect { arg_ownership, .. } = &func.blocks[1].body[0] {
        assert_eq!(arg_ownership, &[ArgOwnership::Borrowed]);
    } else {
        panic!("expected ApplyIndirect");
    }
}

#[test]
fn test_annotate_apply_indirect_zero_user_args() {
    let interner = StringInterner::new();
    let pool = Pool::new();
    let builtins = BuiltinOwnershipSets::empty();

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

    let mut func = make_func_named(func_name, vec![], Idx::NONE, blocks, vec![Idx::INT; 3]);
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
    let builtins = BuiltinOwnershipSets::empty();
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

    let mut func = make_func_named(func_name, vec![], Idx::NONE, blocks, vec![Idx::INT; 5]);
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
    let mut builtins = BuiltinOwnershipSets::empty();
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

    // The consuming override requires `v0` to carry `List<int>`.
    let mut var_types = vec![Idx::INT; 4];
    let list_int = pool.list(Idx::INT);
    var_types[0] = list_int;

    let mut func = make_func_named(func_name, vec![], Idx::NONE, blocks, vec![Idx::INT; 4]);
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

/// Different capture types do not affect the uniform residual ABI.
#[test]
fn test_annotate_apply_indirect_different_capture_types_defaults_borrowed() {
    let interner = StringInterner::new();
    let mut pool = Pool::new();

    let target = interner.intern("concat");
    let func_name = interner.intern("caller");

    // Register concat as consuming_receiver + consuming_second_arg
    // so the type-qualified authority fires for List but not str.
    let mut builtins = BuiltinOwnershipSets::empty();
    builtins.consuming_receiver.insert(target);
    builtins.consuming_second_arg.insert(target);

    // v0: List<int>, v1: str — different types for the capture position.
    // The type-qualified authority fires for List (marks receiver+second Owned)
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

    let mut func = make_func_named(func_name, vec![], Idx::NONE, blocks, vec![Idx::INT; 7]);
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
            "capture types must not affect the borrowed residual ABI"
        );
    } else {
        panic!("expected ApplyIndirect");
    }
}

/// Same-typed capture provenance does not specialize residual ownership.
#[test]
fn same_capture_types_do_not_specialize_indirect_ownership() {
    let interner = StringInterner::new();
    let pool = Pool::new();
    let builtins = BuiltinOwnershipSets::empty();

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

    let mut func = make_func_named(func_name, vec![], Idx::NONE, blocks, vec![Idx::INT; 7]);
    let mut sigs = FxHashMap::default();
    sigs.insert(target, make_sig(&[Ownership::Owned, Ownership::Owned]));

    super::annotate_arg_ownership(&mut func, &sigs, &interner, &builtins, &pool);

    if let ArcInstr::ApplyIndirect { arg_ownership, .. } = &func.blocks[2].body[0] {
        assert_eq!(
            arg_ownership,
            &[ArgOwnership::Borrowed],
            "SSA merge facts are devirtualization-only, not ownership policy"
        );
    } else {
        panic!("expected ApplyIndirect");
    }
}

/// Cross-instantiation closure provenance does not specialize ownership.
#[test]
fn cross_instantiation_closures_keep_borrowed_indirect_ownership() {
    let interner = StringInterner::new();
    let mut pool = Pool::new();

    let target = interner.intern("concat");
    let func_name = interner.intern("caller");

    let mut builtins = BuiltinOwnershipSets::empty();
    builtins.consuming_receiver.insert(target);
    builtins.consuming_second_arg.insert(target);

    let list_int = pool.list(Idx::INT);
    let list_str = pool.list(Idx::STR);

    let blocks = vec![
        ArcBlock {
            id: ArcBlockId::new(0),
            params: vec![],
            body: vec![ArcInstr::PartialApply {
                dst: ArcVarId::new(2),
                ty: Idx::NONE,
                func: target,
                args: vec![ArcVarId::new(0)], // captures List<int>
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
                args: vec![ArcVarId::new(1)], // captures List<str>
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

    let mut func = make_func_named(func_name, vec![], Idx::NONE, blocks, vec![Idx::INT; 7]);
    func.var_types[0] = list_int; // List<int>
    func.var_types[1] = list_str; // List<str>

    let mut sigs = FxHashMap::default();
    sigs.insert(target, make_sig(&[Ownership::Owned, Ownership::Owned]));

    super::annotate_arg_ownership(&mut func, &sigs, &interner, &builtins, &pool);

    if let ArcInstr::ApplyIndirect { arg_ownership, .. } = &func.blocks[2].body[0] {
        assert_eq!(
            arg_ownership,
            &[ArgOwnership::Borrowed],
            "type-tag agreement must not change the borrowed residual ABI"
        );
    } else {
        panic!("expected ApplyIndirect");
    }
}

/// Diamond CFG provenance does not participate in ownership policy.
#[test]
fn diamond_cfg_closure_provenance_keeps_borrowed_abi() {
    let interner = StringInterner::new();
    let pool = Pool::new();
    let builtins = BuiltinOwnershipSets::empty();

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

    let mut func = make_func_named(func_name, vec![], Idx::NONE, blocks, vec![Idx::INT; 7]);
    let mut sigs = FxHashMap::default();
    sigs.insert(target, make_sig(&[Ownership::Owned, Ownership::Owned]));

    super::annotate_arg_ownership(&mut func, &sigs, &interner, &builtins, &pool);

    if let ArcInstr::ApplyIndirect { arg_ownership, .. } = &func.blocks[3].body[0] {
        assert_eq!(
            arg_ownership,
            &[ArgOwnership::Borrowed],
            "CFG provenance must not change the residual closure-call ABI"
        );
    } else {
        panic!("expected ApplyIndirect");
    }
}

// Protocol builtin consumer tests — verify annotate_arg_ownership produces
// correct ArgOwnership vectors when encountering protocol builtin callees.

/// Verify `annotate_arg_ownership` maps `__index` to [Borrowed, Borrowed].
/// This is the consumer directly responsible for the original `__index` RC leak.
#[test]
fn annotate_protocol_index_produces_borrowed_vector() {
    let interner = StringInterner::new();
    let pool = Pool::new();
    let builtins = BuiltinOwnershipSets::new(&interner);
    let func_name = interner.intern("caller");
    let index_name = interner.intern("__index");

    let blocks = vec![ArcBlock {
        id: ArcBlockId::new(0),
        params: vec![],
        body: vec![ArcInstr::Apply {
            dst: ArcVarId::new(2),
            ty: Idx::INT,
            func: index_name,
            args: vec![ArcVarId::new(0), ArcVarId::new(1)],
            arg_ownership: vec![ArgOwnership::Owned; 2], // pre-annotation default
            mono_instance_id: None,
        }],
        terminator: ArcTerminator::Return {
            value: ArcVarId::new(2),
        },
    }];

    let mut func = make_func_named(func_name, vec![], Idx::NONE, blocks, vec![Idx::INT; 3]);
    super::annotate_arg_ownership(
        &mut func,
        &FxHashMap::default(),
        &interner,
        &builtins,
        &pool,
    );

    if let ArcInstr::Apply { arg_ownership, .. } = &func.blocks[0].body[0] {
        assert_eq!(
            arg_ownership,
            &[ArgOwnership::Borrowed, ArgOwnership::Borrowed],
            "__index must produce [Borrowed, Borrowed]"
        );
    } else {
        panic!("expected Apply");
    }
}

/// Verify `annotate_arg_ownership` maps `__iter_next` to [Owned, Borrowed].
#[test]
fn annotate_protocol_iter_next_produces_owned_borrowed() {
    let interner = StringInterner::new();
    let pool = Pool::new();
    let builtins = BuiltinOwnershipSets::new(&interner);
    let func_name = interner.intern("caller");
    let iter_next_name = interner.intern("__iter_next");

    let blocks = vec![ArcBlock {
        id: ArcBlockId::new(0),
        params: vec![],
        body: vec![ArcInstr::Apply {
            dst: ArcVarId::new(2),
            ty: Idx::INT,
            func: iter_next_name,
            args: vec![ArcVarId::new(0), ArcVarId::new(1)],
            arg_ownership: vec![ArgOwnership::Owned; 2],
            mono_instance_id: None,
        }],
        terminator: ArcTerminator::Return {
            value: ArcVarId::new(2),
        },
    }];

    let mut func = make_func_named(func_name, vec![], Idx::NONE, blocks, vec![Idx::INT; 3]);
    super::annotate_arg_ownership(
        &mut func,
        &FxHashMap::default(),
        &interner,
        &builtins,
        &pool,
    );

    if let ArcInstr::Apply { arg_ownership, .. } = &func.blocks[0].body[0] {
        assert_eq!(
            arg_ownership,
            &[ArgOwnership::Owned, ArgOwnership::Borrowed],
            "__iter_next must produce [Owned, Borrowed]"
        );
    } else {
        panic!("expected Apply");
    }
}

/// Verify `annotate_arg_ownership` maps `ori_iter_drop` to [Owned].
#[test]
fn annotate_protocol_iter_drop_produces_owned() {
    let interner = StringInterner::new();
    let pool = Pool::new();
    let builtins = BuiltinOwnershipSets::new(&interner);
    let func_name = interner.intern("caller");
    let iter_drop_name = interner.intern("ori_iter_drop");

    let blocks = vec![ArcBlock {
        id: ArcBlockId::new(0),
        params: vec![],
        body: vec![ArcInstr::Apply {
            dst: ArcVarId::new(1),
            ty: Idx::UNIT,
            func: iter_drop_name,
            args: vec![ArcVarId::new(0)],
            arg_ownership: vec![ArgOwnership::Owned],
            mono_instance_id: None,
        }],
        terminator: ArcTerminator::Return {
            value: ArcVarId::new(1),
        },
    }];

    let mut func = make_func_named(func_name, vec![], Idx::NONE, blocks, vec![Idx::INT; 2]);
    super::annotate_arg_ownership(
        &mut func,
        &FxHashMap::default(),
        &interner,
        &builtins,
        &pool,
    );

    if let ArcInstr::Apply { arg_ownership, .. } = &func.blocks[0].body[0] {
        assert_eq!(
            arg_ownership,
            &[ArgOwnership::Owned],
            "ori_iter_drop must produce [Owned]"
        );
    } else {
        panic!("expected Apply");
    }
}

/// Verify `annotate_arg_ownership` maps `__collect_set` to [Owned].
#[test]
fn annotate_protocol_collect_set_produces_owned() {
    let interner = StringInterner::new();
    let pool = Pool::new();
    let builtins = BuiltinOwnershipSets::new(&interner);
    let func_name = interner.intern("caller");
    let collect_name = interner.intern("__collect_set");

    let blocks = vec![ArcBlock {
        id: ArcBlockId::new(0),
        params: vec![],
        body: vec![ArcInstr::Apply {
            dst: ArcVarId::new(1),
            ty: Idx::INT,
            func: collect_name,
            args: vec![ArcVarId::new(0)],
            arg_ownership: vec![ArgOwnership::Owned],
            mono_instance_id: None,
        }],
        terminator: ArcTerminator::Return {
            value: ArcVarId::new(1),
        },
    }];

    let mut func = make_func_named(func_name, vec![], Idx::NONE, blocks, vec![Idx::INT; 2]);
    super::annotate_arg_ownership(
        &mut func,
        &FxHashMap::default(),
        &interner,
        &builtins,
        &pool,
    );

    if let ArcInstr::Apply { arg_ownership, .. } = &func.blocks[0].body[0] {
        assert_eq!(
            arg_ownership,
            &[ArgOwnership::Owned],
            "__collect_set must produce [Owned]"
        );
    } else {
        panic!("expected Apply");
    }
}

/// Verify `annotate_arg_ownership` maps `iter` to [Borrowed] for non-collection receivers.
///
/// The protocol definition says `Iter.arg_ownership() = [Borrowed]` — this is the base case
/// for generic/primitive receivers. Collection receivers get overridden to Owned by
/// the typed authority — see `annotate_iter_on_collection_overrides_to_owned`.
#[test]
fn annotate_protocol_iter_produces_borrowed() {
    let interner = StringInterner::new();
    let pool = Pool::new();
    let builtins = BuiltinOwnershipSets::new(&interner);
    let func_name = interner.intern("caller");
    let iter_name = interner.intern("iter");

    let blocks = vec![ArcBlock {
        id: ArcBlockId::new(0),
        params: vec![],
        body: vec![ArcInstr::Apply {
            dst: ArcVarId::new(1),
            ty: Idx::INT,
            func: iter_name,
            args: vec![ArcVarId::new(0)],
            arg_ownership: vec![ArgOwnership::Owned],
            mono_instance_id: None,
        }],
        terminator: ArcTerminator::Return {
            value: ArcVarId::new(1),
        },
    }];

    let mut func = make_func_named(func_name, vec![], Idx::NONE, blocks, vec![Idx::INT; 2]);
    super::annotate_arg_ownership(
        &mut func,
        &FxHashMap::default(),
        &interner,
        &builtins,
        &pool,
    );

    if let ArcInstr::Apply { arg_ownership, .. } = &func.blocks[0].body[0] {
        assert_eq!(
            arg_ownership,
            &[ArgOwnership::Borrowed],
            "iter must produce [Borrowed]"
        );
    } else {
        panic!("expected Apply");
    }
}

/// Verify `annotate_arg_ownership` overrides `iter` to [Owned] for collection receivers.
///
/// The protocol defines `Iter.arg_ownership() = [Borrowed]` as the base case, but
/// the typed authority promotes collection receivers (List/Map/Set) to Owned
/// because the runtime transfers buffer ownership to the iterator. This test verifies
/// the full `annotate_arg_ownership` flow — protocol base + type-qualified override.
///
/// Prior to this test, the override path was never exercised at the consumer level
/// (the original test used a non-collection receiver type).
#[test]
fn annotate_iter_on_collection_overrides_to_owned() {
    let interner = StringInterner::new();
    let mut pool = Pool::new();
    let builtins = BuiltinOwnershipSets::new(&interner);
    let func_name = interner.intern("caller");
    let iter_name = interner.intern("iter");

    let list_int = pool.list(Idx::INT);

    let blocks = vec![ArcBlock {
        id: ArcBlockId::new(0),
        params: vec![],
        body: vec![ArcInstr::Apply {
            dst: ArcVarId::new(1),
            ty: Idx::INT,
            func: iter_name,
            args: vec![ArcVarId::new(0)],
            arg_ownership: vec![ArgOwnership::Owned],
            mono_instance_id: None,
        }],
        terminator: ArcTerminator::Return {
            value: ArcVarId::new(1),
        },
    }];

    // var 0 = List<int>, var 1 = int (return)
    let mut func = make_func_named(
        func_name,
        vec![],
        Idx::NONE,
        blocks,
        vec![list_int, Idx::INT],
    );
    super::annotate_arg_ownership(
        &mut func,
        &FxHashMap::default(),
        &interner,
        &builtins,
        &pool,
    );

    if let ArcInstr::Apply { arg_ownership, .. } = &func.blocks[0].body[0] {
        assert_eq!(
            arg_ownership,
            &[ArgOwnership::Owned],
            "iter on List<int> must produce [Owned] via consuming override"
        );
    } else {
        panic!("expected Apply");
    }
}
