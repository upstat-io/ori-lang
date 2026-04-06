use super::*;
use crate::ir::{ArcBlock, ArcBlockId, ArcInstr, ArcTerminator, ArcValue, LitValue, RcStrategy};
use crate::test_helpers::{b, make_func_named, owned_param, v};
use ori_ir::StringInterner;
use ori_types::Idx;

// Helper to build a simple Let instruction (placeholder for computed values).
fn let_int(dst: u32) -> ArcInstr {
    ArcInstr::Let {
        dst: v(dst),
        ty: Idx::INT,
        value: ArcValue::Literal(LitValue::Int(0)),
    }
}

fn let_bool(dst: u32) -> ArcInstr {
    ArcInstr::Let {
        dst: v(dst),
        ty: Idx::BOOL,
        value: ArcValue::Literal(LitValue::Bool(false)),
    }
}

fn apply(dst: u32, func: Name, args: Vec<ArcVarId>) -> ArcInstr {
    ArcInstr::Apply {
        dst: v(dst),
        ty: Idx::INT,
        func,
        args,
        arg_ownership: vec![],
    }
}

fn rc_dec(var: u32) -> ArcInstr {
    ArcInstr::RcDec {
        var: v(var),
        strategy: RcStrategy::HeapPointer,
    }
}

#[test]
fn scalar_self_recursive_tail_call_detected() {
    // gcd(a, b) = if b == 0 then a else gcd(b, a % b)
    //
    // bb0: Branch (b == 0) ? bb1 : bb2
    // bb1: Jump bb3(a)            — base case
    // bb2: %rem = a % b
    //      %r = Apply @gcd(b, rem)
    //      Jump bb3(%r)           — recursive case
    // bb3(%ret): Return %ret      — merge block
    let interner = StringInterner::new();
    let gcd = interner.intern("gcd");

    let blocks = vec![
        ArcBlock {
            id: b(0),
            params: vec![],
            body: vec![let_bool(2)],
            terminator: ArcTerminator::Branch {
                cond: v(2),
                then_block: b(1),
                else_block: b(2),
            },
        },
        ArcBlock {
            id: b(1),
            params: vec![],
            body: vec![],
            terminator: ArcTerminator::Jump {
                target: b(3),
                args: vec![v(0)],
            },
        },
        ArcBlock {
            id: b(2),
            params: vec![],
            body: vec![let_int(3), apply(4, gcd, vec![v(1), v(3)])],
            terminator: ArcTerminator::Jump {
                target: b(3),
                args: vec![v(4)],
            },
        },
        ArcBlock {
            id: b(3),
            params: vec![(v(5), Idx::INT)],
            body: vec![],
            terminator: ArcTerminator::Return { value: v(5) },
        },
    ];

    let func = make_func_named(
        gcd,
        vec![owned_param(0, Idx::INT), owned_param(1, Idx::INT)],
        Idx::INT,
        blocks,
        vec![Idx::INT; 6],
    );

    let sites = detect_tail_calls(&func);
    assert_eq!(sites.len(), 1);
    assert_eq!(sites[0].call_block, b(2));
    assert_eq!(sites[0].kind, TailCallKind::Apply { instr_idx: 1 });
}

#[test]
fn tail_call_with_safe_rc_decs_detected() {
    // f(x) = if done then 0 else { let y = ...; f(y); RcDec(x) }
    // RcDec(x) is safe because x is NOT in Apply args.
    //
    // bb0: Branch ? bb1 : bb2
    // bb1: Jump bb3(base)
    // bb2: %y = ...; %r = Apply @f(y); RcDec(x); Jump bb3(%r)
    // bb3(%ret): Return %ret
    let interner = StringInterner::new();
    let f = interner.intern("f");

    let blocks = vec![
        ArcBlock {
            id: b(0),
            params: vec![],
            body: vec![let_bool(1)],
            terminator: ArcTerminator::Branch {
                cond: v(1),
                then_block: b(1),
                else_block: b(2),
            },
        },
        ArcBlock {
            id: b(1),
            params: vec![],
            body: vec![let_int(2)],
            terminator: ArcTerminator::Jump {
                target: b(3),
                args: vec![v(2)],
            },
        },
        ArcBlock {
            id: b(2),
            params: vec![],
            body: vec![
                let_int(3),
                apply(4, f, vec![v(3)]),
                rc_dec(0), // RcDec(x) — x is NOT in Apply args, safe
            ],
            terminator: ArcTerminator::Jump {
                target: b(3),
                args: vec![v(4)],
            },
        },
        ArcBlock {
            id: b(3),
            params: vec![(v(5), Idx::INT)],
            body: vec![],
            terminator: ArcTerminator::Return { value: v(5) },
        },
    ];

    let func = make_func_named(
        f,
        vec![owned_param(0, Idx::INT)],
        Idx::INT,
        blocks,
        vec![Idx::INT; 6],
    );

    let sites = detect_tail_calls(&func);
    assert_eq!(sites.len(), 1);
    assert_eq!(sites[0].call_block, b(2));
    assert_eq!(sites[0].kind, TailCallKind::Apply { instr_idx: 1 });
}

#[test]
fn non_tail_call_result_transformed() {
    // f(n) = f(n - 1) + 1  — result used in addition, NOT a tail call.
    //
    // bb0: %r = Apply @f(n-1); %out = r + 1; Return %out
    let interner = StringInterner::new();
    let f = interner.intern("f");

    let blocks = vec![ArcBlock {
        id: b(0),
        params: vec![],
        body: vec![
            apply(1, f, vec![v(0)]),
            let_int(2), // represents v(1) + 1
        ],
        terminator: ArcTerminator::Return { value: v(2) },
    }];

    let func = make_func_named(
        f,
        vec![owned_param(0, Idx::INT)],
        Idx::INT,
        blocks,
        vec![Idx::INT; 3],
    );

    let sites = detect_tail_calls(&func);
    assert!(
        sites.is_empty(),
        "result used in addition should not be detected"
    );
}

#[test]
fn mutual_recursion_not_detected() {
    // f(n) calls g(n), not itself.
    let interner = StringInterner::new();
    let f = interner.intern("f");
    let g = interner.intern("g");

    let blocks = vec![ArcBlock {
        id: b(0),
        params: vec![],
        body: vec![apply(1, g, vec![v(0)])],
        terminator: ArcTerminator::Return { value: v(1) },
    }];

    let func = make_func_named(
        f,
        vec![owned_param(0, Idx::INT)],
        Idx::INT,
        blocks,
        vec![Idx::INT; 2],
    );

    let sites = detect_tail_calls(&func);
    assert!(sites.is_empty(), "mutual recursion should not be detected");
}

#[test]
fn rc_dec_target_used_as_arg_not_eligible() {
    // f(x) calls f(x) then RcDec(x) — unsafe, x is both arg and dec target.
    //
    // bb0: Branch ? bb1 : bb2
    // bb1: Jump bb3(base)
    // bb2: %r = Apply @f(x); RcDec(x); Jump bb3(%r)
    // bb3(%ret): Return %ret
    let interner = StringInterner::new();
    let f = interner.intern("f");

    let blocks = vec![
        ArcBlock {
            id: b(0),
            params: vec![],
            body: vec![let_bool(1)],
            terminator: ArcTerminator::Branch {
                cond: v(1),
                then_block: b(1),
                else_block: b(2),
            },
        },
        ArcBlock {
            id: b(1),
            params: vec![],
            body: vec![let_int(2)],
            terminator: ArcTerminator::Jump {
                target: b(3),
                args: vec![v(2)],
            },
        },
        ArcBlock {
            id: b(2),
            params: vec![],
            body: vec![
                apply(3, f, vec![v(0)]),
                rc_dec(0), // RcDec(x) — x IS in Apply args, UNSAFE
            ],
            terminator: ArcTerminator::Jump {
                target: b(3),
                args: vec![v(3)],
            },
        },
        ArcBlock {
            id: b(3),
            params: vec![(v(4), Idx::INT)],
            body: vec![],
            terminator: ArcTerminator::Return { value: v(4) },
        },
    ];

    let func = make_func_named(
        f,
        vec![owned_param(0, Idx::INT)],
        Idx::INT,
        blocks,
        vec![Idx::INT; 5],
    );

    let sites = detect_tail_calls(&func);
    assert!(
        sites.is_empty(),
        "RcDec target in Apply args should exclude tail call"
    );
}

#[test]
fn apply_indirect_not_detected() {
    // Closure call (ApplyIndirect) in tail position — callee unknown.
    //
    // bb0: %r = ApplyIndirect(closure, args); Return %r
    let interner = StringInterner::new();
    let f = interner.intern("f");

    let blocks = vec![ArcBlock {
        id: b(0),
        params: vec![],
        body: vec![ArcInstr::ApplyIndirect {
            dst: v(1),
            ty: Idx::INT,
            closure: v(0),
            args: vec![],
            arg_ownership: vec![],
        }],
        terminator: ArcTerminator::Return { value: v(1) },
    }];

    let func = make_func_named(
        f,
        vec![owned_param(0, Idx::INT)],
        Idx::INT,
        blocks,
        vec![Idx::INT; 2],
    );

    let sites = detect_tail_calls(&func);
    assert!(
        sites.is_empty(),
        "ApplyIndirect should not be detected as tail call"
    );
}

#[test]
fn multi_clause_function_detected() {
    // Multi-clause: @f(0) = 1; @f(n) = n * f(n-1)
    // Lowered to a single function with match dispatch.
    // The recursive call f(n-1) is in a match arm block.
    //
    // bb0: Branch (n == 0) ? bb1 : bb2
    // bb1: Jump bb3(1)              — base clause
    // bb2: %r = Apply @f(n-1)
    //      %out = n * r             — NOT tail call (result transformed)
    //      Jump bb3(%out)
    // bb3(%ret): Return %ret
    //
    // This tests that transformed results in match arms are NOT detected.
    let interner = StringInterner::new();
    let f = interner.intern("f");

    let blocks = vec![
        ArcBlock {
            id: b(0),
            params: vec![],
            body: vec![let_bool(1)],
            terminator: ArcTerminator::Branch {
                cond: v(1),
                then_block: b(1),
                else_block: b(2),
            },
        },
        ArcBlock {
            id: b(1),
            params: vec![],
            body: vec![let_int(2)],
            terminator: ArcTerminator::Jump {
                target: b(3),
                args: vec![v(2)],
            },
        },
        ArcBlock {
            id: b(2),
            params: vec![],
            body: vec![
                apply(3, f, vec![v(0)]),
                let_int(4), // n * f(n-1) — transforms result
            ],
            terminator: ArcTerminator::Jump {
                target: b(3),
                args: vec![v(4)], // passes transformed value, not Apply result
            },
        },
        ArcBlock {
            id: b(3),
            params: vec![(v(5), Idx::INT)],
            body: vec![],
            terminator: ArcTerminator::Return { value: v(5) },
        },
    ];

    let func = make_func_named(
        f,
        vec![owned_param(0, Idx::INT)],
        Idx::INT,
        blocks,
        vec![Idx::INT; 6],
    );

    let sites = detect_tail_calls(&func);
    assert!(
        sites.is_empty(),
        "transformed result in match arm should not be detected"
    );
}

#[test]
fn multi_clause_tail_recursive_arm_detected() {
    // Multi-clause where one arm IS tail-recursive:
    // @f(0) = 1; @f(n) = f(n - 1)   (identity recursion, contrived but valid)
    //
    // bb0: Branch (n == 0) ? bb1 : bb2
    // bb1: Jump bb3(1)
    // bb2: %r = Apply @f(n-1); Jump bb3(%r)
    // bb3(%ret): Return %ret
    let interner = StringInterner::new();
    let f = interner.intern("f");

    let blocks = vec![
        ArcBlock {
            id: b(0),
            params: vec![],
            body: vec![let_bool(1)],
            terminator: ArcTerminator::Branch {
                cond: v(1),
                then_block: b(1),
                else_block: b(2),
            },
        },
        ArcBlock {
            id: b(1),
            params: vec![],
            body: vec![let_int(2)],
            terminator: ArcTerminator::Jump {
                target: b(3),
                args: vec![v(2)],
            },
        },
        ArcBlock {
            id: b(2),
            params: vec![],
            body: vec![let_int(3), apply(4, f, vec![v(3)])],
            terminator: ArcTerminator::Jump {
                target: b(3),
                args: vec![v(4)],
            },
        },
        ArcBlock {
            id: b(3),
            params: vec![(v(5), Idx::INT)],
            body: vec![],
            terminator: ArcTerminator::Return { value: v(5) },
        },
    ];

    let func = make_func_named(
        f,
        vec![owned_param(0, Idx::INT)],
        Idx::INT,
        blocks,
        vec![Idx::INT; 6],
    );

    let sites = detect_tail_calls(&func);
    assert_eq!(sites.len(), 1);
    assert_eq!(sites[0].call_block, b(2));
}

#[test]
fn direct_tail_call_same_block() {
    // Unusual case: Apply and Return in the same block (no merge block).
    //
    // bb0: %r = Apply @f(args); Return %r
    let interner = StringInterner::new();
    let f = interner.intern("f");

    let blocks = vec![ArcBlock {
        id: b(0),
        params: vec![],
        body: vec![apply(1, f, vec![v(0)])],
        terminator: ArcTerminator::Return { value: v(1) },
    }];

    let func = make_func_named(
        f,
        vec![owned_param(0, Idx::INT)],
        Idx::INT,
        blocks,
        vec![Idx::INT; 2],
    );

    let sites = detect_tail_calls(&func);
    assert_eq!(sites.len(), 1);
    assert_eq!(sites[0].call_block, b(0));
    assert_eq!(sites[0].kind, TailCallKind::Apply { instr_idx: 0 });
}

#[test]
fn no_tail_call_in_non_recursive_function() {
    // f(x) = x + 1  — no recursion at all.
    let blocks = vec![ArcBlock {
        id: b(0),
        params: vec![],
        body: vec![let_int(1)],
        terminator: ArcTerminator::Return { value: v(1) },
    }];

    let func = make_func_named(
        Name::from_raw(1),
        vec![owned_param(0, Idx::INT)],
        Idx::INT,
        blocks,
        vec![Idx::INT; 2],
    );

    let sites = detect_tail_calls(&func);
    assert!(sites.is_empty());
}

#[test]
fn non_rc_dec_instruction_after_apply_blocks_detection() {
    // Apply followed by a Let (not RcDec) before the Jump — not eligible.
    //
    // bb0: Branch ? bb1 : bb2
    // bb1: Jump bb3(base)
    // bb2: %r = Apply @f(x); %side = let 0; Jump bb3(%r)
    // bb3(%ret): Return %ret
    let interner = StringInterner::new();
    let f = interner.intern("f");

    let blocks = vec![
        ArcBlock {
            id: b(0),
            params: vec![],
            body: vec![let_bool(1)],
            terminator: ArcTerminator::Branch {
                cond: v(1),
                then_block: b(1),
                else_block: b(2),
            },
        },
        ArcBlock {
            id: b(1),
            params: vec![],
            body: vec![let_int(2)],
            terminator: ArcTerminator::Jump {
                target: b(3),
                args: vec![v(2)],
            },
        },
        ArcBlock {
            id: b(2),
            params: vec![],
            body: vec![
                apply(3, f, vec![v(0)]),
                let_int(4), // non-RcDec instruction after Apply
            ],
            terminator: ArcTerminator::Jump {
                target: b(3),
                args: vec![v(3)], // passes Apply result (not v(4))
            },
        },
        ArcBlock {
            id: b(3),
            params: vec![(v(5), Idx::INT)],
            body: vec![],
            terminator: ArcTerminator::Return { value: v(5) },
        },
    ];

    let func = make_func_named(
        f,
        vec![owned_param(0, Idx::INT)],
        Idx::INT,
        blocks,
        vec![Idx::INT; 6],
    );

    let sites = detect_tail_calls(&func);
    assert!(
        sites.is_empty(),
        "non-RcDec after Apply should block detection"
    );
}

// Rewrite tests

/// Helper: detect + rewrite in one step.
fn detect_and_rewrite(func: &mut ArcFunction) {
    func.tail_calls = detect_tail_calls(func);
    rewrite_tail_calls(func);
}

/// Check that no block body contains an Apply to `func_name`.
fn has_self_apply(func: &ArcFunction, func_name: Name) -> bool {
    func.blocks.iter().any(|block| {
        block
            .body
            .iter()
            .any(|i| matches!(i, ArcInstr::Apply { func, .. } if *func == func_name))
    })
}

#[test]
fn rewrite_creates_loop_structure() {
    // gcd(a, b) — same structure as scalar_self_recursive_tail_call_detected
    let interner = StringInterner::new();
    let gcd = interner.intern("gcd");

    let blocks = vec![
        ArcBlock {
            id: b(0),
            params: vec![],
            body: vec![let_bool(2)],
            terminator: ArcTerminator::Branch {
                cond: v(2),
                then_block: b(1),
                else_block: b(2),
            },
        },
        ArcBlock {
            id: b(1),
            params: vec![],
            body: vec![],
            terminator: ArcTerminator::Jump {
                target: b(3),
                args: vec![v(0)],
            },
        },
        ArcBlock {
            id: b(2),
            params: vec![],
            body: vec![let_int(3), apply(4, gcd, vec![v(1), v(3)])],
            terminator: ArcTerminator::Jump {
                target: b(3),
                args: vec![v(4)],
            },
        },
        ArcBlock {
            id: b(3),
            params: vec![(v(5), Idx::INT)],
            body: vec![],
            terminator: ArcTerminator::Return { value: v(5) },
        },
    ];

    let mut func = make_func_named(
        gcd,
        vec![owned_param(0, Idx::INT), owned_param(1, Idx::INT)],
        Idx::INT,
        blocks,
        vec![Idx::INT; 6],
    );

    detect_and_rewrite(&mut func);

    // Entry is the new trampoline (not the original bb0).
    assert_ne!(func.entry, ArcBlockId::new(0));

    // Trampoline jumps to the header (old entry, bb0).
    let trampoline = &func.blocks[func.entry.index()];
    let ArcTerminator::Jump { target, args } = &trampoline.terminator else {
        panic!("trampoline should have Jump terminator");
    };
    assert_eq!(*target, ArcBlockId::new(0));
    assert_eq!(args, &[v(0), v(1)]);

    // Header (old entry) has block params with FRESH var IDs (not func param IDs).
    // This prevents block merge Phase 7 from mistaking the trampoline's
    // initial-value pass for a self-referencing back-edge.
    let header = &func.blocks[0];
    assert_eq!(header.params.len(), 2);
    assert_ne!(
        header.params[0].0,
        v(0),
        "header param must use fresh var ID"
    );
    assert_ne!(
        header.params[1].0,
        v(1),
        "header param must use fresh var ID"
    );
    assert_eq!(header.params[0].1, Idx::INT);
    assert_eq!(header.params[1].1, Idx::INT);

    // Header body starts with Let bindings: original param vars ← fresh block params.
    assert!(matches!(
        &header.body[0],
        ArcInstr::Let { dst, value: ArcValue::Var(src), .. }
            if *dst == v(0) && *src == header.params[0].0
    ));
    assert!(matches!(
        &header.body[1],
        ArcInstr::Let { dst, value: ArcValue::Var(src), .. }
            if *dst == v(1) && *src == header.params[1].0
    ));

    // Tail-call block (bb2) now jumps to header with call args, not to merge.
    let tail_block = &func.blocks[2];
    let ArcTerminator::Jump { target, args } = &tail_block.terminator else {
        panic!("tail call block should have Jump terminator");
    };
    assert_eq!(*target, ArcBlockId::new(0)); // back-edge to header
    assert_eq!(args, &[v(1), v(3)]); // gcd(b, rem) args

    // No Apply @gcd remains in any block.
    assert!(!has_self_apply(&func, gcd));

    // tail_calls consumed (emptied).
    assert!(func.tail_calls.is_empty());
}

#[test]
fn rewrite_preserves_rc_decs() {
    // f(x) with RcDec(x) after tail call — RcDec should survive the rewrite.
    let interner = StringInterner::new();
    let f = interner.intern("f");

    let blocks = vec![
        ArcBlock {
            id: b(0),
            params: vec![],
            body: vec![let_bool(1)],
            terminator: ArcTerminator::Branch {
                cond: v(1),
                then_block: b(1),
                else_block: b(2),
            },
        },
        ArcBlock {
            id: b(1),
            params: vec![],
            body: vec![let_int(2)],
            terminator: ArcTerminator::Jump {
                target: b(3),
                args: vec![v(2)],
            },
        },
        ArcBlock {
            id: b(2),
            params: vec![],
            body: vec![
                let_int(3),
                apply(4, f, vec![v(3)]),
                rc_dec(0), // safe RcDec — target not in call args
            ],
            terminator: ArcTerminator::Jump {
                target: b(3),
                args: vec![v(4)],
            },
        },
        ArcBlock {
            id: b(3),
            params: vec![(v(5), Idx::INT)],
            body: vec![],
            terminator: ArcTerminator::Return { value: v(5) },
        },
    ];

    let mut func = make_func_named(
        f,
        vec![owned_param(0, Idx::INT)],
        Idx::INT,
        blocks,
        vec![Idx::INT; 6],
    );

    detect_and_rewrite(&mut func);

    // Tail-call block should still have the RcDec.
    let tail_block = &func.blocks[2];
    let has_rc_dec = tail_block
        .body
        .iter()
        .any(|i| matches!(i, ArcInstr::RcDec { var, .. } if *var == v(0)));
    assert!(has_rc_dec, "RcDec should be preserved after rewrite");

    // Apply should be removed.
    let has_apply = tail_block
        .body
        .iter()
        .any(|i| matches!(i, ArcInstr::Apply { .. }));
    assert!(!has_apply, "Apply should be removed after rewrite");

    // Terminator should be back-edge to header.
    let ArcTerminator::Jump { target, args } = &tail_block.terminator else {
        panic!("expected Jump terminator");
    };
    assert_eq!(*target, ArcBlockId::new(0));
    assert_eq!(args, &[v(3)]);
}

#[test]
fn rewrite_handles_direct_tail_call() {
    // Apply + Return in same block (no merge block).
    // f(x) = f(x) (infinite loop, but structurally valid)
    let interner = StringInterner::new();
    let f = interner.intern("f");

    let blocks = vec![ArcBlock {
        id: b(0),
        params: vec![],
        body: vec![apply(1, f, vec![v(0)])],
        terminator: ArcTerminator::Return { value: v(1) },
    }];

    let mut func = make_func_named(
        f,
        vec![owned_param(0, Idx::INT)],
        Idx::INT,
        blocks,
        vec![Idx::INT; 2],
    );

    detect_and_rewrite(&mut func);

    // Apply removed from header.
    assert!(!has_self_apply(&func, f));

    // Header has fresh block param and Let binding bridging to original.
    let header = &func.blocks[0];
    assert_eq!(header.params.len(), 1);
    assert_ne!(
        header.params[0].0,
        v(0),
        "header param must use fresh var ID"
    );

    // Header body starts with Let binding: v(0) ← fresh block param.
    assert!(matches!(
        &header.body[0],
        ArcInstr::Let { dst, value: ArcValue::Var(src), .. }
            if *dst == v(0) && *src == header.params[0].0
    ));

    // Header terminator is now Jump(header, args) — a self-loop.
    let ArcTerminator::Jump { target, args } = &header.terminator else {
        panic!("expected Jump terminator on header");
    };
    assert_eq!(*target, ArcBlockId::new(0));
    assert_eq!(args, &[v(0)]);
}

#[test]
fn rewrite_does_not_modify_non_tail_function() {
    // f(x) = x + 1 — no recursion, no changes.
    let blocks = vec![ArcBlock {
        id: b(0),
        params: vec![],
        body: vec![let_int(1)],
        terminator: ArcTerminator::Return { value: v(1) },
    }];

    let mut func = make_func_named(
        Name::from_raw(1),
        vec![owned_param(0, Idx::INT)],
        Idx::INT,
        blocks,
        vec![Idx::INT; 2],
    );

    let original_block_count = func.blocks.len();
    detect_and_rewrite(&mut func);

    // No trampoline created, no structural changes.
    assert_eq!(func.blocks.len(), original_block_count);
    assert_eq!(func.entry, ArcBlockId::new(0));
}

#[test]
fn rewrite_handles_multiple_tail_call_sites() {
    // f(a, b) = match ... { arm1 -> f(x, y), arm2 -> f(p, q), arm3 -> base }
    //
    // bb0: Switch ? bb1 / bb2 / bb3
    // bb1: %r1 = Apply @f(v10, v11); Jump bb4(%r1)
    // bb2: %r2 = Apply @f(v20, v21); Jump bb4(%r2)
    // bb3: Jump bb4(base)
    // bb4(%ret): Return %ret
    let interner = StringInterner::new();
    let f = interner.intern("f");

    let blocks = vec![
        ArcBlock {
            id: b(0),
            params: vec![],
            body: vec![let_int(2)],
            terminator: ArcTerminator::Switch {
                scrutinee: v(2),
                cases: vec![(0, b(1)), (1, b(2))],
                default: b(3),
            },
        },
        ArcBlock {
            id: b(1),
            params: vec![],
            body: vec![let_int(3), let_int(4), apply(5, f, vec![v(3), v(4)])],
            terminator: ArcTerminator::Jump {
                target: b(4),
                args: vec![v(5)],
            },
        },
        ArcBlock {
            id: b(2),
            params: vec![],
            body: vec![let_int(6), let_int(7), apply(8, f, vec![v(6), v(7)])],
            terminator: ArcTerminator::Jump {
                target: b(4),
                args: vec![v(8)],
            },
        },
        ArcBlock {
            id: b(3),
            params: vec![],
            body: vec![let_int(9)],
            terminator: ArcTerminator::Jump {
                target: b(4),
                args: vec![v(9)],
            },
        },
        ArcBlock {
            id: b(4),
            params: vec![(v(10), Idx::INT)],
            body: vec![],
            terminator: ArcTerminator::Return { value: v(10) },
        },
    ];

    let mut func = make_func_named(
        f,
        vec![owned_param(0, Idx::INT), owned_param(1, Idx::INT)],
        Idx::INT,
        blocks,
        vec![Idx::INT; 11],
    );

    detect_and_rewrite(&mut func);

    // Both tail call blocks should now jump to header.
    let header_id = ArcBlockId::new(0);
    for block_idx in [1, 2] {
        let ArcTerminator::Jump { target, .. } = &func.blocks[block_idx].terminator else {
            panic!("block {block_idx} should have Jump terminator");
        };
        assert_eq!(
            *target, header_id,
            "block {block_idx} should back-edge to header"
        );
    }

    // No Apply @f remains.
    assert!(!has_self_apply(&func, f));

    // Base case block (bb3) still jumps to merge (bb4), unchanged.
    let ArcTerminator::Jump { target, .. } = &func.blocks[3].terminator else {
        panic!("base case block should have Jump terminator");
    };
    assert_eq!(*target, ArcBlockId::new(4));
}

// Invoke tail call tests — user function calls are lowered as Invoke terminators

fn invoke_block(
    id: u32,
    body: Vec<ArcInstr>,
    dst: u32,
    func: Name,
    args: Vec<ArcVarId>,
    normal: u32,
    unwind: u32,
) -> ArcBlock {
    ArcBlock {
        id: b(id),
        params: vec![],
        body,
        terminator: ArcTerminator::Invoke {
            dst: v(dst),
            ty: Idx::INT,
            func,
            args,
            arg_ownership: vec![],
            normal: b(normal),
            unwind: b(unwind),
        },
    }
}

#[test]
fn invoke_tail_call_cross_block_detected() {
    // gcd(a, b) lowered with Invoke (real-world pattern):
    //
    // bb0: Branch (b == 0) ? bb1 : bb2
    // bb1: Jump bb3(a)
    // bb2: Invoke @gcd(b, rem) → normal: bb4, unwind: bb5
    // bb3(%ret): Return %ret
    // bb4: Jump bb3(%dst)
    // bb5: Resume
    let interner = StringInterner::new();
    let gcd = interner.intern("gcd");

    let blocks = vec![
        ArcBlock {
            id: b(0),
            params: vec![],
            body: vec![let_bool(2), let_int(3)],
            terminator: ArcTerminator::Branch {
                cond: v(2),
                then_block: b(1),
                else_block: b(2),
            },
        },
        ArcBlock {
            id: b(1),
            params: vec![],
            body: vec![],
            terminator: ArcTerminator::Jump {
                target: b(3),
                args: vec![v(0)],
            },
        },
        invoke_block(2, vec![], 4, gcd, vec![v(1), v(3)], 4, 5),
        ArcBlock {
            id: b(3),
            params: vec![(v(5), Idx::INT)],
            body: vec![],
            terminator: ArcTerminator::Return { value: v(5) },
        },
        ArcBlock {
            id: b(4),
            params: vec![],
            body: vec![],
            terminator: ArcTerminator::Jump {
                target: b(3),
                args: vec![v(4)],
            },
        },
        ArcBlock {
            id: b(5),
            params: vec![],
            body: vec![],
            terminator: ArcTerminator::Resume,
        },
    ];

    let func = make_func_named(
        gcd,
        vec![owned_param(0, Idx::INT), owned_param(1, Idx::INT)],
        Idx::INT,
        blocks,
        vec![Idx::INT; 6],
    );

    let sites = detect_tail_calls(&func);
    assert_eq!(sites.len(), 1);
    assert_eq!(sites[0].call_block, b(2));
    assert_eq!(sites[0].kind, TailCallKind::Invoke);
}

#[test]
fn invoke_tail_call_with_rc_decs_in_normal_block() {
    // Invoke with RcDec in normal block — still eligible if decs don't
    // target the invoke args.
    //
    // bb0: Branch ? bb1 : bb2
    // bb1: Jump bb3(base)
    // bb2: Invoke @f(y) → normal: bb4, unwind: bb5
    // bb3(%ret): Return %ret
    // bb4: RcDec(x); Jump bb3(%dst)   — x not in invoke args, safe
    // bb5: Resume
    let interner = StringInterner::new();
    let f = interner.intern("f");

    let blocks = vec![
        ArcBlock {
            id: b(0),
            params: vec![],
            body: vec![let_bool(1), let_int(2)],
            terminator: ArcTerminator::Branch {
                cond: v(1),
                then_block: b(1),
                else_block: b(2),
            },
        },
        ArcBlock {
            id: b(1),
            params: vec![],
            body: vec![let_int(3)],
            terminator: ArcTerminator::Jump {
                target: b(3),
                args: vec![v(3)],
            },
        },
        invoke_block(2, vec![], 4, f, vec![v(2)], 4, 5),
        ArcBlock {
            id: b(3),
            params: vec![(v(5), Idx::INT)],
            body: vec![],
            terminator: ArcTerminator::Return { value: v(5) },
        },
        ArcBlock {
            id: b(4),
            params: vec![],
            body: vec![rc_dec(0)], // RcDec(x) — x not in invoke args
            terminator: ArcTerminator::Jump {
                target: b(3),
                args: vec![v(4)],
            },
        },
        ArcBlock {
            id: b(5),
            params: vec![],
            body: vec![],
            terminator: ArcTerminator::Resume,
        },
    ];

    let func = make_func_named(
        f,
        vec![owned_param(0, Idx::INT)],
        Idx::INT,
        blocks,
        vec![Idx::INT; 6],
    );

    let sites = detect_tail_calls(&func);
    assert_eq!(sites.len(), 1);
    assert_eq!(sites[0].kind, TailCallKind::Invoke);
}

#[test]
fn invoke_tail_call_unsafe_rc_dec_rejected() {
    // Invoke where the normal block RcDec targets an invoke arg — unsafe.
    //
    // bb2: Invoke @f(x) → normal: bb4, unwind: bb5
    // bb4: RcDec(x); Jump bb3(%dst)   — x IS in invoke args, UNSAFE
    let interner = StringInterner::new();
    let f = interner.intern("f");

    let blocks = vec![
        ArcBlock {
            id: b(0),
            params: vec![],
            body: vec![let_bool(1)],
            terminator: ArcTerminator::Branch {
                cond: v(1),
                then_block: b(1),
                else_block: b(2),
            },
        },
        ArcBlock {
            id: b(1),
            params: vec![],
            body: vec![let_int(2)],
            terminator: ArcTerminator::Jump {
                target: b(3),
                args: vec![v(2)],
            },
        },
        invoke_block(2, vec![], 3, f, vec![v(0)], 4, 5),
        ArcBlock {
            id: b(3),
            params: vec![(v(4), Idx::INT)],
            body: vec![],
            terminator: ArcTerminator::Return { value: v(4) },
        },
        ArcBlock {
            id: b(4),
            params: vec![],
            body: vec![rc_dec(0)], // RcDec(x=v(0)) — v(0) IS in invoke args
            terminator: ArcTerminator::Jump {
                target: b(3),
                args: vec![v(3)],
            },
        },
        ArcBlock {
            id: b(5),
            params: vec![],
            body: vec![],
            terminator: ArcTerminator::Resume,
        },
    ];

    let func = make_func_named(
        f,
        vec![owned_param(0, Idx::INT)],
        Idx::INT,
        blocks,
        vec![Idx::INT; 5],
    );

    let sites = detect_tail_calls(&func);
    assert!(
        sites.is_empty(),
        "RcDec target in invoke args should exclude tail call"
    );
}

#[test]
fn invoke_rewrite_creates_loop_structure() {
    // Same as invoke_tail_call_cross_block_detected but also tests rewrite.
    let interner = StringInterner::new();
    let gcd = interner.intern("gcd");

    let blocks = vec![
        ArcBlock {
            id: b(0),
            params: vec![],
            body: vec![let_bool(2), let_int(3)],
            terminator: ArcTerminator::Branch {
                cond: v(2),
                then_block: b(1),
                else_block: b(2),
            },
        },
        ArcBlock {
            id: b(1),
            params: vec![],
            body: vec![],
            terminator: ArcTerminator::Jump {
                target: b(3),
                args: vec![v(0)],
            },
        },
        invoke_block(2, vec![], 4, gcd, vec![v(1), v(3)], 4, 5),
        ArcBlock {
            id: b(3),
            params: vec![(v(5), Idx::INT)],
            body: vec![],
            terminator: ArcTerminator::Return { value: v(5) },
        },
        ArcBlock {
            id: b(4),
            params: vec![],
            body: vec![],
            terminator: ArcTerminator::Jump {
                target: b(3),
                args: vec![v(4)],
            },
        },
        ArcBlock {
            id: b(5),
            params: vec![],
            body: vec![],
            terminator: ArcTerminator::Resume,
        },
    ];

    let mut func = make_func_named(
        gcd,
        vec![owned_param(0, Idx::INT), owned_param(1, Idx::INT)],
        Idx::INT,
        blocks,
        vec![Idx::INT; 6],
    );

    detect_and_rewrite(&mut func);

    // Tail call block (bb2) now jumps to header with invoke args.
    let tail_block = &func.blocks[2];
    let ArcTerminator::Jump { target, args } = &tail_block.terminator else {
        panic!("tail call block should have Jump terminator after rewrite");
    };
    assert_eq!(*target, ArcBlockId::new(0));
    assert_eq!(args, &[v(1), v(3)]);

    // No Invoke remains for self-recursive calls.
    let has_invoke = func.blocks.iter().any(|block| {
        matches!(
            &block.terminator,
            ArcTerminator::Invoke { func, .. } if *func == gcd
        )
    });
    assert!(!has_invoke, "self-recursive Invoke should be removed");
}

#[test]
fn invoke_rewrite_moves_rc_decs_from_normal_block() {
    // Invoke with RcDec in normal block — rewrite should move decs to call block.
    let interner = StringInterner::new();
    let f = interner.intern("f");

    let blocks = vec![
        ArcBlock {
            id: b(0),
            params: vec![],
            body: vec![let_bool(1), let_int(2)],
            terminator: ArcTerminator::Branch {
                cond: v(1),
                then_block: b(1),
                else_block: b(2),
            },
        },
        ArcBlock {
            id: b(1),
            params: vec![],
            body: vec![let_int(3)],
            terminator: ArcTerminator::Jump {
                target: b(3),
                args: vec![v(3)],
            },
        },
        invoke_block(2, vec![], 4, f, vec![v(2)], 4, 5),
        ArcBlock {
            id: b(3),
            params: vec![(v(5), Idx::INT)],
            body: vec![],
            terminator: ArcTerminator::Return { value: v(5) },
        },
        ArcBlock {
            id: b(4),
            params: vec![],
            body: vec![rc_dec(0)], // will be moved to call block
            terminator: ArcTerminator::Jump {
                target: b(3),
                args: vec![v(4)],
            },
        },
        ArcBlock {
            id: b(5),
            params: vec![],
            body: vec![],
            terminator: ArcTerminator::Resume,
        },
    ];

    let mut func = make_func_named(
        f,
        vec![owned_param(0, Idx::INT)],
        Idx::INT,
        blocks,
        vec![Idx::INT; 6],
    );

    detect_and_rewrite(&mut func);

    // Call block (bb2) should now contain the RcDec from the normal block.
    let call_block = &func.blocks[2];
    let has_rc_dec = call_block
        .body
        .iter()
        .any(|i| matches!(i, ArcInstr::RcDec { var, .. } if *var == v(0)));
    assert!(
        has_rc_dec,
        "RcDec should be moved from normal block to call block"
    );

    // Normal block (bb4) body should be empty (decs were drained).
    assert!(
        func.blocks[4].body.is_empty(),
        "normal block body should be empty after drain"
    );
}

// Pre-emission tail-position analysis (Section 12.2)

#[test]
fn constant_stack_non_recursive_is_false() {
    // f(x) = x + 1 — no recursion, constant stack.
    let blocks = vec![ArcBlock {
        id: b(0),
        params: vec![],
        body: vec![let_int(1)],
        terminator: ArcTerminator::Return { value: v(1) },
    }];

    let func = make_func_named(
        Name::from_raw(1),
        vec![owned_param(0, Idx::INT)],
        Idx::INT,
        blocks,
        vec![Idx::INT; 2],
    );

    // Empty SCC peers → not in recursive SCC.
    let scc_peers = rustc_hash::FxHashSet::default();
    assert!(
        !has_non_tail_recursive_calls(&func, &scc_peers),
        "non-recursive function should have constant stack"
    );
}

#[test]
fn constant_stack_tail_recursive_is_false() {
    // gcd(a, b) — all self-recursive calls in tail position.
    let interner = StringInterner::new();
    let gcd = interner.intern("gcd");

    let blocks = vec![
        ArcBlock {
            id: b(0),
            params: vec![],
            body: vec![let_bool(2)],
            terminator: ArcTerminator::Branch {
                cond: v(2),
                then_block: b(1),
                else_block: b(2),
            },
        },
        ArcBlock {
            id: b(1),
            params: vec![],
            body: vec![],
            terminator: ArcTerminator::Jump {
                target: b(3),
                args: vec![v(0)],
            },
        },
        ArcBlock {
            id: b(2),
            params: vec![],
            body: vec![let_int(3), apply(4, gcd, vec![v(1), v(3)])],
            terminator: ArcTerminator::Jump {
                target: b(3),
                args: vec![v(4)],
            },
        },
        ArcBlock {
            id: b(3),
            params: vec![(v(5), Idx::INT)],
            body: vec![],
            terminator: ArcTerminator::Return { value: v(5) },
        },
    ];

    let func = make_func_named(
        gcd,
        vec![owned_param(0, Idx::INT), owned_param(1, Idx::INT)],
        Idx::INT,
        blocks,
        vec![Idx::INT; 6],
    );

    let scc_peers: rustc_hash::FxHashSet<Name> = [gcd].into_iter().collect();
    assert!(
        !has_non_tail_recursive_calls(&func, &scc_peers),
        "tail-recursive function should have constant stack"
    );
}

#[test]
fn constant_stack_non_tail_recursive_is_true() {
    // f(n) = f(n - 1) + 1 — recursive call result used in addition.
    let interner = StringInterner::new();
    let f = interner.intern("f");

    let blocks = vec![ArcBlock {
        id: b(0),
        params: vec![],
        body: vec![
            apply(1, f, vec![v(0)]),
            let_int(2), // represents v(1) + 1
        ],
        terminator: ArcTerminator::Return { value: v(2) },
    }];

    let func = make_func_named(
        f,
        vec![owned_param(0, Idx::INT)],
        Idx::INT,
        blocks,
        vec![Idx::INT; 3],
    );

    let scc_peers: rustc_hash::FxHashSet<Name> = [f].into_iter().collect();
    assert!(
        has_non_tail_recursive_calls(&func, &scc_peers),
        "non-tail recursive function should have unbounded stack"
    );
}

#[test]
fn constant_stack_mutual_recursion_non_tail_is_true() {
    // f(n) calls g(n) in non-tail position.
    // Both f and g are in the same SCC.
    let interner = StringInterner::new();
    let f = interner.intern("f");
    let g = interner.intern("g");

    let blocks = vec![ArcBlock {
        id: b(0),
        params: vec![],
        body: vec![
            apply(1, g, vec![v(0)]),
            let_int(2), // transforms g's result
        ],
        terminator: ArcTerminator::Return { value: v(2) },
    }];

    let func = make_func_named(
        f,
        vec![owned_param(0, Idx::INT)],
        Idx::INT,
        blocks,
        vec![Idx::INT; 3],
    );

    let scc_peers: rustc_hash::FxHashSet<Name> = [f, g].into_iter().collect();
    assert!(
        has_non_tail_recursive_calls(&func, &scc_peers),
        "mutual recursion with non-tail call should have unbounded stack"
    );
}

#[test]
fn constant_stack_mutual_recursion_tail_is_false() {
    // f(n) calls g(n) in tail position.
    // Both f and g are in the same SCC.
    let interner = StringInterner::new();
    let f = interner.intern("f");
    let g = interner.intern("g");

    let blocks = vec![ArcBlock {
        id: b(0),
        params: vec![],
        body: vec![apply(1, g, vec![v(0)])],
        terminator: ArcTerminator::Return { value: v(1) },
    }];

    let func = make_func_named(
        f,
        vec![owned_param(0, Idx::INT)],
        Idx::INT,
        blocks,
        vec![Idx::INT; 2],
    );

    let scc_peers: rustc_hash::FxHashSet<Name> = [f, g].into_iter().collect();
    assert!(
        !has_non_tail_recursive_calls(&func, &scc_peers),
        "mutual recursion with tail call should have constant stack"
    );
}

#[test]
fn constant_stack_invoke_tail_is_false() {
    // Invoke in tail position (normal → Return).
    let interner = StringInterner::new();
    let f = interner.intern("f");

    let blocks = vec![
        invoke_block(0, vec![], 1, f, vec![v(0)], 1, 2),
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
    ];

    let func = make_func_named(
        f,
        vec![owned_param(0, Idx::INT)],
        Idx::INT,
        blocks,
        vec![Idx::INT; 2],
    );

    let scc_peers: rustc_hash::FxHashSet<Name> = [f].into_iter().collect();
    assert!(
        !has_non_tail_recursive_calls(&func, &scc_peers),
        "invoke in tail position should have constant stack"
    );
}

#[test]
fn constant_stack_invoke_cross_block_tail_is_false() {
    // Invoke → normal → Jump → merge(Return).
    let interner = StringInterner::new();
    let gcd = interner.intern("gcd");

    let blocks = vec![
        ArcBlock {
            id: b(0),
            params: vec![],
            body: vec![let_bool(2)],
            terminator: ArcTerminator::Branch {
                cond: v(2),
                then_block: b(1),
                else_block: b(2),
            },
        },
        ArcBlock {
            id: b(1),
            params: vec![],
            body: vec![],
            terminator: ArcTerminator::Jump {
                target: b(3),
                args: vec![v(0)],
            },
        },
        invoke_block(2, vec![], 3, gcd, vec![v(1)], 4, 5),
        ArcBlock {
            id: b(3),
            params: vec![(v(4), Idx::INT)],
            body: vec![],
            terminator: ArcTerminator::Return { value: v(4) },
        },
        ArcBlock {
            id: b(4),
            params: vec![],
            body: vec![],
            terminator: ArcTerminator::Jump {
                target: b(3),
                args: vec![v(3)],
            },
        },
        ArcBlock {
            id: b(5),
            params: vec![],
            body: vec![],
            terminator: ArcTerminator::Resume,
        },
    ];

    let func = make_func_named(
        gcd,
        vec![owned_param(0, Idx::INT), owned_param(1, Idx::INT)],
        Idx::INT,
        blocks,
        vec![Idx::INT; 5],
    );

    let scc_peers: rustc_hash::FxHashSet<Name> = [gcd].into_iter().collect();
    assert!(
        !has_non_tail_recursive_calls(&func, &scc_peers),
        "invoke in cross-block tail position should have constant stack"
    );
}
