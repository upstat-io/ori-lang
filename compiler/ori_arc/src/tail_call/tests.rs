use super::*;
use crate::ir::{ArcBlock, ArcInstr, ArcTerminator, ArcValue, LitValue, RcStrategy};
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
    assert_eq!(sites[0].call_instr_idx, 1);
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
    assert_eq!(sites[0].call_instr_idx, 1);
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
    assert_eq!(sites[0].call_instr_idx, 0);
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
