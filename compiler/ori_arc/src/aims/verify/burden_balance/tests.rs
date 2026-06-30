//! Tests for VF-1 burden-balance check (function-exit net basic form).

use ori_ir::Name;
use ori_types::Idx;

use crate::aims::verify::burden_balance::verify_burden_balance;
use crate::ir::{ArcBlock, ArcBlockId, ArcInstr, ArcTerminator};
use crate::test_helpers::{b, make_func, v};
use crate::verify::BurdenBalanceError;

/// Construct an `ArcFunction` over a single var, populating
/// `burden_emitted` for `v(0)` and assigning the provided blocks.
fn make_burden_func(blocks: Vec<ArcBlock>) -> crate::ir::ArcFunction {
    let var_types = vec![Idx::from_raw(0)];
    let mut func = make_func(Vec::new(), Idx::from_raw(0), blocks, var_types);
    func.name = Name::from_raw(1);
    // Mark v(0) as a burden-emitted variable.
    func.burden_emitted = vec![true];
    func
}

/// Build a block at id `n` with body and `Return(value)` terminator over `v(0)`.
fn return_block(n: u32, body: Vec<ArcInstr>) -> ArcBlock {
    ArcBlock {
        id: b(n),
        params: Vec::new(),
        body,
        terminator: ArcTerminator::Return { value: v(0) },
    }
}

/// Build a block that jumps to `target` with no body.
fn jump_block(n: u32, body: Vec<ArcInstr>, target: ArcBlockId) -> ArcBlock {
    ArcBlock {
        id: b(n),
        params: Vec::new(),
        body,
        terminator: ArcTerminator::Jump {
            target,
            args: Vec::new(),
        },
    }
}

/// Build a block that branches on `cond` (v(0) as scalar stand-in) to two targets.
fn branch_block(n: u32, then_b: ArcBlockId, else_b: ArcBlockId) -> ArcBlock {
    ArcBlock {
        id: b(n),
        params: Vec::new(),
        body: Vec::new(),
        terminator: ArcTerminator::Branch {
            cond: v(0),
            then_block: then_b,
            else_block: else_b,
        },
    }
}

// ITEM-4 — Positive pin: balanced IR passes.

#[test]
fn balanced_straight_line_passes() {
    // entry: BurdenInc(v0); Jump exit
    // exit: BurdenDec(v0); Return v0
    let entry = jump_block(0, vec![ArcInstr::BurdenInc { var: v(0) }], b(1));
    let exit = return_block(1, vec![ArcInstr::BurdenDec { var: v(0) }]);
    let func = make_burden_func(vec![entry, exit]);

    let errors = verify_burden_balance(&func);
    assert!(
        errors.is_empty(),
        "balanced straight-line burden IR should pass; got: {errors:?}"
    );
}

// ITEM-5 — Negative pin: unbalanced IR fails.

#[test]
fn unbalanced_straight_line_fails() {
    // entry: BurdenInc(v0); Return v0 (no matching dec → net = 1 at exit)
    let entry = return_block(0, vec![ArcInstr::BurdenInc { var: v(0) }]);
    let func = make_burden_func(vec![entry]);

    let errors = verify_burden_balance(&func);
    assert_eq!(
        errors.len(),
        1,
        "unbalanced straight-line burden IR should produce exactly one error; got: {errors:?}"
    );
    let BurdenBalanceError {
        var,
        expected_net,
        observed_net,
        exit_block,
        ..
    } = errors[0].clone();
    assert_eq!(var, v(0));
    assert_eq!(expected_net, 0);
    assert_eq!(observed_net, 1);
    assert_eq!(exit_block, b(0));
}

// ITEM-6 — CFG-diamond predecessor disagreement pin (5 blocks).

#[test]
fn diamond_predecessor_disagreement_fails() {
    // 5-block diamond:
    //   entry (b0): Branch → then (b1) | else (b2)
    //   then (b1): BurdenInc(v0); Jump merge (b3)
    //   else (b2): /* no op */ Jump merge (b3)
    //   merge (b3): Jump exit (b4)
    //   exit (b4): Return v0
    //
    // Predecessor exit nets at b3: from b1 = +1, from b2 = 0 → disagreement.
    let entry = branch_block(0, b(1), b(2));
    let then_b = jump_block(1, vec![ArcInstr::BurdenInc { var: v(0) }], b(3));
    let else_b = jump_block(2, Vec::new(), b(3));
    let merge = jump_block(3, Vec::new(), b(4));
    let exit = return_block(4, Vec::new());
    let func = make_burden_func(vec![entry, then_b, else_b, merge, exit]);

    let errors = verify_burden_balance(&func);
    assert_eq!(
        errors.len(),
        1,
        "diamond predecessor disagreement should produce exactly one error; got: {errors:?}"
    );
    let BurdenBalanceError {
        var,
        expected_net,
        exit_block,
        ..
    } = errors[0].clone();
    assert_eq!(var, v(0));
    assert_eq!(expected_net, 0);
    // The reported exit_block is the merge block where predecessors disagreed.
    assert_eq!(exit_block, b(3));
}

// Coverage: empty burden_emitted is a fast-path no-op.

#[test]
fn empty_burden_emitted_returns_empty() {
    let entry = return_block(0, Vec::new());
    let mut func = make_burden_func(vec![entry]);
    func.burden_emitted = Vec::new();
    let errors = verify_burden_balance(&func);
    assert!(errors.is_empty());
}

// Coverage: BurdenDecPartial mirrors BurdenDec (delta = -1).

#[test]
fn burden_dec_partial_balances_inc() {
    let entry = jump_block(0, vec![ArcInstr::BurdenInc { var: v(0) }], b(1));
    let exit = return_block(
        1,
        vec![ArcInstr::BurdenDecPartial {
            var: v(0),
            skip_fields: Vec::new(),
        }],
    );
    let func = make_burden_func(vec![entry, exit]);
    let errors = verify_burden_balance(&func);
    assert!(
        errors.is_empty(),
        "BurdenDecPartial should balance BurdenInc on whole-var basis; got: {errors:?}"
    );
}

// Verify-gate fail-closed pin on a cyclic disagreeing loop.
// A loop whose body carries a non-zero per-iteration whole-var burden delta has
// no static entry net; freeze-on-disagree CONVERGES the loop-header merge to a
// recorded disagreement (rather than oscillating to the iteration cap), and
// `verify_burden_balance` reports it as a HARD `merge-disagree` error
// (fail-closed) rather than silently passing on stale nets. Pre-cure
// (`balance_verdict` returning `violation: None` on the non-converged path) this
// returned ZERO errors — the under-verification this verifier exists to eliminate.
// This pins the MergeDisagree path; the `ConvergenceExhausted` fail-closed branch
// — unreachable through any real CFG once freeze-on-disagree lands — is pinned
// directly against a hand-built non-converged net in `burden_delta/tests.rs`.
#[test]
fn cyclic_disagree_loop_verify_reports_merge_disagree() {
    //   b0 entry  : Jump b1
    //   b1 header : Branch b2 | b3
    //   b2 body   : BurdenInc(v0); Jump b1   (back-edge, non-zero cycle delta)
    //   b3 exit   : Return v0
    let entry = jump_block(0, Vec::new(), b(1));
    let header = branch_block(1, b(2), b(3));
    let body = jump_block(2, vec![ArcInstr::BurdenInc { var: v(0) }], b(1));
    let exit = return_block(3, Vec::new());
    let func = make_burden_func(vec![entry, header, body, exit]);

    let errors = verify_burden_balance(&func);

    assert_eq!(
        errors.len(),
        1,
        "a cyclic disagreeing burden delta MUST fail verification closed; got: {errors:?}"
    );
    assert_eq!(
        errors[0].exit_kind, "merge-disagree",
        "freeze-on-disagree converges the loop header to a recorded merge \
         disagreement, not an iteration-cap exhaustion; got: {:?}",
        errors[0].exit_kind
    );
}

//  TDD matrix — axis-1 Let-Literal slice.
// Per `TF-3` / TF-1 + Clamping.

mod fresh_site_inc_balance {
    mod let_literal {
        use super::super::*;

        #[test]
        fn positive_balanced_let_string_with_scope_exit_dec() {
            // FRESH Let{Literal::String} emits BurdenInc; matching scope-exit
            // BurdenDec balances → net=0 per `TF-3` + §8 RL-2.
            let entry = jump_block(0, vec![ArcInstr::BurdenInc { var: v(0) }], b(1));
            let exit = return_block(1, vec![ArcInstr::BurdenDec { var: v(0) }]);
            let func = make_burden_func(vec![entry, exit]);
            let errors = verify_burden_balance(&func);
            assert!(
                errors.is_empty(),
                "FRESH Let{{String}}+Dec must balance; got: {errors:?}"
            );
        }

        #[test]
        fn negative_missing_dec_let_string_fails() {
            // FRESH Let{Literal::String} emits BurdenInc; missing scope-exit
            // BurdenDec leaves net=1 — VF-1 must surface the imbalance.
            let entry = return_block(0, vec![ArcInstr::BurdenInc { var: v(0) }]);
            let func = make_burden_func(vec![entry]);
            let errors = verify_burden_balance(&func);
            assert_eq!(
                errors.len(),
                1,
                "missing Dec must produce net=1; got: {errors:?}"
            );
            let BurdenBalanceError {
                var,
                observed_net,
                expected_net,
                ..
            } = errors[0].clone();
            assert_eq!((var, observed_net, expected_net), (v(0), 1, 0));
        }

        #[test]
        fn clamp_scalar_let_int_not_inc_emits_no_burden() {
            // Scalar Let{Literal::Int} per `TF-1` → SCALAR state,
            // ITEM-2 directive: scalars MUST NOT receive Inc. Var stays out of
            // burden_emitted; verifier sees zero ops → net=0.
            let entry = return_block(0, Vec::new());
            let mut func = make_burden_func(vec![entry]);
            func.burden_emitted = Vec::new();
            let errors = verify_burden_balance(&func);
            assert!(
                errors.is_empty(),
                "scalar var (no Inc emitted) must pass VF-1; got: {errors:?}"
            );
        }
    }

    // axis-1 Construct slice.
    // Per `TF-3` — Construct emits FRESH(Owned, Linear, Once,
    // Unique, BlockLocal, ReusableCtor, may_alloc=true). Burden lower emits
    // BurdenInc at the Construct site; symmetric BurdenDec required at
    // last-use / scope-exit / Construct-field-store transfer.
    mod construct {
        use super::super::*;

        #[test]
        fn positive_balanced_construct_struct_scope_exit_dec() {
            // axis-2 = scope-exit-only; axis-3 = Affine (dropped without use).
            // Construct fresh struct → BurdenInc; unused at scope-exit →
            // BurdenDec balances.
            let entry = jump_block(0, vec![ArcInstr::BurdenInc { var: v(0) }], b(1));
            let exit = return_block(1, vec![ArcInstr::BurdenDec { var: v(0) }]);
            let func = make_burden_func(vec![entry, exit]);
            let errors = verify_burden_balance(&func);
            assert!(
                errors.is_empty(),
                "Construct+scope-exit Dec must balance; got: {errors:?}"
            );
        }

        #[test]
        fn negative_construct_missing_scope_exit_dec_fails() {
            // axis-2 = scope-exit-only without matching Dec; net=1 imbalance.
            let entry = return_block(0, vec![ArcInstr::BurdenInc { var: v(0) }]);
            let func = make_burden_func(vec![entry]);
            let errors = verify_burden_balance(&func);
            assert_eq!(
                errors.len(),
                1,
                "Construct missing Dec must fail; got: {errors:?}"
            );
            let BurdenBalanceError {
                var,
                observed_net,
                expected_net,
                ..
            } = errors[0].clone();
            assert_eq!((var, observed_net, expected_net), (v(0), 1, 0));
        }

        #[test]
        fn positive_construct_field_store_balanced_via_dec_field() {
            // axis-2 = Construct-field-store; consumer transfers ownership
            // into parent struct field. BurdenDecField targets a sub-field
            // grain (excluded from whole-var balance per burden_balance.rs
            // module doc); whole-var BurdenDec required to balance the Inc.
            let entry = jump_block(
                0,
                vec![
                    ArcInstr::BurdenInc { var: v(0) },
                    ArcInstr::BurdenDecField {
                        base: v(0),
                        field: 0,
                    },
                ],
                b(1),
            );
            let exit = return_block(1, vec![ArcInstr::BurdenDec { var: v(0) }]);
            let func = make_burden_func(vec![entry, exit]);
            let errors = verify_burden_balance(&func);
            assert!(
                errors.is_empty(),
                "Construct + DecField (sub-grain) + scope-exit Dec must balance whole-var; \
                got: {errors:?}"
            );
        }

        #[test]
        fn negative_construct_only_dec_field_fails_whole_var_balance() {
            // axis-2 = Construct-field-store ONLY (no whole-var Dec).
            // BurdenDecField is sub-field grain → does NOT cancel BurdenInc
            // per burden_balance.rs module doc. Whole-var net=1, must fail.
            let entry = return_block(
                0,
                vec![
                    ArcInstr::BurdenInc { var: v(0) },
                    ArcInstr::BurdenDecField {
                        base: v(0),
                        field: 0,
                    },
                ],
            );
            let func = make_burden_func(vec![entry]);
            let errors = verify_burden_balance(&func);
            assert_eq!(
                errors.len(),
                1,
                "DecField alone must not cancel whole-var Inc; got: {errors:?}"
            );
            let BurdenBalanceError {
                var,
                observed_net,
                expected_net,
                ..
            } = errors[0].clone();
            assert_eq!((var, observed_net, expected_net), (v(0), 1, 0));
        }

        #[test]
        fn positive_construct_variant_dec_balances_inc() {
            // axis-1 = Construct(EnumVariant) → BurdenInc on enum value;
            // BurdenDecVariant (whole-var grain per ir/instr.rs)
            // balances on equal footing with BurdenDec / BurdenDecPartial.
            let entry = jump_block(0, vec![ArcInstr::BurdenInc { var: v(0) }], b(1));
            let exit = return_block(1, vec![ArcInstr::BurdenDecVariant { var: v(0) }]);
            let func = make_burden_func(vec![entry, exit]);
            let errors = verify_burden_balance(&func);
            assert!(
                errors.is_empty(),
                "Construct(EnumVariant)+DecVariant must balance whole-var; got: {errors:?}"
            );
        }

        #[test]
        fn positive_construct_diamond_balanced_both_branches() {
            // axis-2 × CFG: Construct in entry → BurdenInc; both branches
            // emit balancing BurdenDec on their respective merges. Confirms
            // multi-block fresh-site balance per VF-1 dataflow.
            let entry = jump_block(0, vec![ArcInstr::BurdenInc { var: v(0) }], b(1));
            let branch = branch_block(1, b(2), b(3));
            let then_b = jump_block(2, vec![ArcInstr::BurdenDec { var: v(0) }], b(4));
            let else_b = jump_block(3, vec![ArcInstr::BurdenDec { var: v(0) }], b(4));
            let merge = return_block(4, Vec::new());
            let func = make_burden_func(vec![entry, branch, then_b, else_b, merge]);
            let errors = verify_burden_balance(&func);
            assert!(
                errors.is_empty(),
                "Construct + balanced-both-branch Dec must pass diamond; got: {errors:?}"
            );
        }

        #[test]
        fn negative_construct_diamond_unbalanced_then_arm_fails() {
            // axis-2 × CFG asymmetric: only then-arm emits Dec → predecessor
            // disagreement at merge; VF-1 surfaces the imbalance.
            let entry = jump_block(0, vec![ArcInstr::BurdenInc { var: v(0) }], b(1));
            let branch = branch_block(1, b(2), b(3));
            let then_b = jump_block(2, vec![ArcInstr::BurdenDec { var: v(0) }], b(4));
            let else_b = jump_block(3, Vec::new(), b(4));
            let merge = return_block(4, Vec::new());
            let func = make_burden_func(vec![entry, branch, then_b, else_b, merge]);
            let errors = verify_burden_balance(&func);
            assert_eq!(
                errors.len(),
                1,
                "Construct + asymmetric Dec must fail predecessor agreement; got: {errors:?}"
            );
        }
    }

    // axis-1 Apply-Owned-return slice.
    // Per `TF-6` — Apply with callee return contract
    // {Unique, MaybeShared} produces an owned non-scalar dst; Burden lower
    // emits BurdenInc at the call site. Symmetric BurdenDec at scope-exit
    // / consumer-transfer mirrors §8 RL-2.
    mod apply_owned_return {
        use super::super::*;

        #[test]
        fn positive_apply_owned_return_scope_exit_dec_balances() {
            // axis-2 = scope-exit-only; axis-3 = Affine (returned value
            // dropped at scope exit without further use).
            let entry = jump_block(0, vec![ArcInstr::BurdenInc { var: v(0) }], b(1));
            let exit = return_block(1, vec![ArcInstr::BurdenDec { var: v(0) }]);
            let func = make_burden_func(vec![entry, exit]);
            let errors = verify_burden_balance(&func);
            assert!(
                errors.is_empty(),
                "Apply-owned-return + Dec must balance; got: {errors:?}"
            );
        }

        #[test]
        fn negative_apply_owned_return_missing_dec_fails() {
            // Apply emits Inc; no Dec → net=1.
            let entry = return_block(0, vec![ArcInstr::BurdenInc { var: v(0) }]);
            let func = make_burden_func(vec![entry]);
            let errors = verify_burden_balance(&func);
            assert_eq!(
                errors.len(),
                1,
                "Apply missing Dec must fail; got: {errors:?}"
            );
            let BurdenBalanceError {
                var,
                observed_net,
                expected_net,
                ..
            } = errors[0].clone();
            assert_eq!((var, observed_net, expected_net), (v(0), 1, 0));
        }

        #[test]
        fn positive_apply_owned_return_passthrough_dec_at_return() {
            // axis-2 = Owned-return-passthrough; FRESH-allocated value
            // returned directly to caller. Per `RL-2`, the
            // returning function still emits the matching Dec at the
            // terminator transfer point (ownership transfers to caller via
            // the Return; symmetric Dec at function-exit preserves VF-1
            // intraprocedural balance).
            let entry = return_block(
                0,
                vec![
                    ArcInstr::BurdenInc { var: v(0) },
                    ArcInstr::BurdenDec { var: v(0) },
                ],
            );
            let func = make_burden_func(vec![entry]);
            let errors = verify_burden_balance(&func);
            assert!(
                errors.is_empty(),
                "Apply-owned-return passthrough + intra-block Dec must balance; got: {errors:?}"
            );
        }

        #[test]
        fn positive_apply_owned_arg_transfer_dec_at_consumer_site() {
            // axis-2 = Owned-arg consumer; FRESH-allocated value flows to
            // Apply with Owned param contract. Burden lower emits Dec at
            // the consuming Apply site per §8 RL-2 (ownership-transferring
            // instruction).
            let entry = jump_block(
                0,
                vec![
                    ArcInstr::BurdenInc { var: v(0) },
                    ArcInstr::BurdenDec { var: v(0) },
                ],
                b(1),
            );
            let exit = return_block(1, Vec::new());
            let func = make_burden_func(vec![entry, exit]);
            let errors = verify_burden_balance(&func);
            assert!(
                errors.is_empty(),
                "Apply + Owned-arg consumer Dec must balance; got: {errors:?}"
            );
        }

        #[test]
        fn clamp_apply_borrowed_arg_no_inc_no_dec_balances() {
            // axis-2 = Borrowed-arg consumer; per `TF-6`
            // refine does not flag Owned for Borrowed return contracts.
            // Burden lower does NOT emit Inc when callee return is Borrowed-
            // shaped (no FRESH-allocation handoff). Verifier sees zero ops
            // → net=0.
            let entry = return_block(0, Vec::new());
            let mut func = make_burden_func(vec![entry]);
            func.burden_emitted = Vec::new();
            let errors = verify_burden_balance(&func);
            assert!(
                errors.is_empty(),
                "Borrowed-return (no Inc emitted) must pass VF-1; got: {errors:?}"
            );
        }

        #[test]
        fn positive_apply_owned_return_partial_dec_balances() {
            // axis-1 × axis-2: Apply-owned-return + BurdenDecPartial at
            // exit. Per burden_balance.rs module doc, BurdenDecPartial
            // mirrors BurdenDec on the whole-var balance grain (delta = -1).
            let entry = jump_block(0, vec![ArcInstr::BurdenInc { var: v(0) }], b(1));
            let exit = return_block(
                1,
                vec![ArcInstr::BurdenDecPartial {
                    var: v(0),
                    skip_fields: vec![0],
                }],
            );
            let func = make_burden_func(vec![entry, exit]);
            let errors = verify_burden_balance(&func);
            assert!(
                errors.is_empty(),
                "Apply-owned-return + DecPartial must balance whole-var; got: {errors:?}"
            );
        }

        #[test]
        fn negative_apply_owned_double_inc_single_dec_fails() {
            // axis-3 = Many cardinality interaction: two Apply sites both
            // returning fresh-owned into v(0) (Inc emitted twice) but only
            // one Dec → net=1 imbalance.
            let entry = jump_block(
                0,
                vec![
                    ArcInstr::BurdenInc { var: v(0) },
                    ArcInstr::BurdenInc { var: v(0) },
                ],
                b(1),
            );
            let exit = return_block(1, vec![ArcInstr::BurdenDec { var: v(0) }]);
            let func = make_burden_func(vec![entry, exit]);
            let errors = verify_burden_balance(&func);
            assert_eq!(
                errors.len(),
                1,
                "Two Incs + one Dec must surface net=1 imbalance; got: {errors:?}"
            );
            let BurdenBalanceError {
                var,
                observed_net,
                expected_net,
                ..
            } = errors[0].clone();
            assert_eq!((var, observed_net, expected_net), (v(0), 1, 0));
        }
    }

    // axis-1 PartialApply slice.
    // Per `TF-7` — PartialApply emits FRESH(NonReusable);
    // captures via capture_state_update (TF-13). Burden lower emits
    // BurdenInc on closure dst. Symmetric BurdenDec at scope-exit /
    // last-use mirrors §8 RL-2.
    mod partial_apply {
        use super::super::*;

        #[test]
        fn positive_partial_apply_scope_exit_dec_balances() {
            // axis-2 = scope-exit-only; axis-3 = Affine.
            // PartialApply emits BurdenInc on closure dst; scope-exit Dec
            // balances → net=0.
            let entry = jump_block(0, vec![ArcInstr::BurdenInc { var: v(0) }], b(1));
            let exit = return_block(1, vec![ArcInstr::BurdenDec { var: v(0) }]);
            let func = make_burden_func(vec![entry, exit]);
            let errors = verify_burden_balance(&func);
            assert!(
                errors.is_empty(),
                "PartialApply + Dec must balance; got: {errors:?}"
            );
        }

        #[test]
        fn negative_partial_apply_missing_dec_fails() {
            // PartialApply emits Inc; missing Dec → net=1.
            let entry = return_block(0, vec![ArcInstr::BurdenInc { var: v(0) }]);
            let func = make_burden_func(vec![entry]);
            let errors = verify_burden_balance(&func);
            assert_eq!(
                errors.len(),
                1,
                "PartialApply missing Dec must fail; got: {errors:?}"
            );
            let BurdenBalanceError {
                var,
                observed_net,
                expected_net,
                ..
            } = errors[0].clone();
            assert_eq!((var, observed_net, expected_net), (v(0), 1, 0));
        }

        #[test]
        fn positive_partial_apply_invoke_consumer_dec_at_call() {
            // axis-2 = Owned-arg consumer (closure invoked / consumed).
            // PartialApply Inc + consuming-Invoke Dec balances within the
            // emitting block.
            let entry = jump_block(
                0,
                vec![
                    ArcInstr::BurdenInc { var: v(0) },
                    ArcInstr::BurdenDec { var: v(0) },
                ],
                b(1),
            );
            let exit = return_block(1, Vec::new());
            let func = make_burden_func(vec![entry, exit]);
            let errors = verify_burden_balance(&func);
            assert!(
                errors.is_empty(),
                "PartialApply + Invoke consumer Dec must balance; got: {errors:?}"
            );
        }

        #[test]
        fn positive_partial_apply_returned_as_closure_dec_at_return() {
            // axis-2 = Owned-return-passthrough; PartialApply emits closure,
            // function returns it to caller. Per §8 RL-2 terminator
            // transfer, intra-block Dec at function-exit preserves VF-1.
            let entry = return_block(
                0,
                vec![
                    ArcInstr::BurdenInc { var: v(0) },
                    ArcInstr::BurdenDec { var: v(0) },
                ],
            );
            let func = make_burden_func(vec![entry]);
            let errors = verify_burden_balance(&func);
            assert!(
                errors.is_empty(),
                "PartialApply returned-as-closure + Dec at return must balance; got: {errors:?}"
            );
        }

        #[test]
        fn negative_partial_apply_diamond_only_then_arm_dec_fails() {
            // axis-2 × CFG: PartialApply at entry, only then-branch decs
            // → predecessor disagreement at merge.
            let entry = jump_block(0, vec![ArcInstr::BurdenInc { var: v(0) }], b(1));
            let branch = branch_block(1, b(2), b(3));
            let then_b = jump_block(2, vec![ArcInstr::BurdenDec { var: v(0) }], b(4));
            let else_b = jump_block(3, Vec::new(), b(4));
            let merge = return_block(4, Vec::new());
            let func = make_burden_func(vec![entry, branch, then_b, else_b, merge]);
            let errors = verify_burden_balance(&func);
            assert_eq!(
                errors.len(),
                1,
                "PartialApply + asymmetric-branch Dec must fail predecessor agreement; \
                got: {errors:?}"
            );
        }

        #[test]
        fn positive_partial_apply_balanced_diamond_both_branches() {
            // axis-2 × CFG mirror: both branches emit Dec → merge agrees.
            let entry = jump_block(0, vec![ArcInstr::BurdenInc { var: v(0) }], b(1));
            let branch = branch_block(1, b(2), b(3));
            let then_b = jump_block(2, vec![ArcInstr::BurdenDec { var: v(0) }], b(4));
            let else_b = jump_block(3, vec![ArcInstr::BurdenDec { var: v(0) }], b(4));
            let merge = return_block(4, Vec::new());
            let func = make_burden_func(vec![entry, branch, then_b, else_b, merge]);
            let errors = verify_burden_balance(&func);
            assert!(
                errors.is_empty(),
                "PartialApply + balanced-both-branch Dec must pass diamond; got: {errors:?}"
            );
        }
    }

    // axis-1 Reuse slice.
    // Per `TF-9` — Reuse emits FRESH(shape) from Reset
    // token's original Construct shape. Burden lower emits BurdenInc at
    // the Reuse site; symmetric BurdenDec mirrors §8 RL-2.
    mod reuse {
        use super::super::*;

        #[test]
        fn positive_reuse_scope_exit_dec_balances() {
            // axis-2 = scope-exit-only; Reuse fresh-allocates new struct
            // from reset token. Scope-exit Dec balances.
            let entry = jump_block(0, vec![ArcInstr::BurdenInc { var: v(0) }], b(1));
            let exit = return_block(1, vec![ArcInstr::BurdenDec { var: v(0) }]);
            let func = make_burden_func(vec![entry, exit]);
            let errors = verify_burden_balance(&func);
            assert!(
                errors.is_empty(),
                "Reuse + Dec must balance; got: {errors:?}"
            );
        }

        #[test]
        fn negative_reuse_missing_dec_fails() {
            // Reuse emits Inc; missing Dec → net=1.
            let entry = return_block(0, vec![ArcInstr::BurdenInc { var: v(0) }]);
            let func = make_burden_func(vec![entry]);
            let errors = verify_burden_balance(&func);
            assert_eq!(
                errors.len(),
                1,
                "Reuse missing Dec must fail; got: {errors:?}"
            );
            let BurdenBalanceError {
                var,
                observed_net,
                expected_net,
                ..
            } = errors[0].clone();
            assert_eq!((var, observed_net, expected_net), (v(0), 1, 0));
        }

        #[test]
        fn positive_reuse_field_store_balanced_via_whole_var_dec() {
            // axis-2 = Construct-field-store (Reuse'd value embedded into
            // parent ctor field). DecField sub-grain + whole-var Dec
            // balances. Mirrors construct::positive_construct_field_store.
            let entry = jump_block(
                0,
                vec![
                    ArcInstr::BurdenInc { var: v(0) },
                    ArcInstr::BurdenDecField {
                        base: v(0),
                        field: 0,
                    },
                ],
                b(1),
            );
            let exit = return_block(1, vec![ArcInstr::BurdenDec { var: v(0) }]);
            let func = make_burden_func(vec![entry, exit]);
            let errors = verify_burden_balance(&func);
            assert!(
                errors.is_empty(),
                "Reuse + DecField (sub-grain) + scope-exit Dec must balance; got: {errors:?}"
            );
        }

        #[test]
        fn positive_reuse_variant_dec_balances_inc() {
            // axis-1 = Reuse(EnumVariant) — Reset/Reuse on enum value;
            // DecVariant balances whole-var grain per ir/instr.rs.
            let entry = jump_block(0, vec![ArcInstr::BurdenInc { var: v(0) }], b(1));
            let exit = return_block(1, vec![ArcInstr::BurdenDecVariant { var: v(0) }]);
            let func = make_burden_func(vec![entry, exit]);
            let errors = verify_burden_balance(&func);
            assert!(
                errors.is_empty(),
                "Reuse(EnumVariant) + DecVariant must balance whole-var; got: {errors:?}"
            );
        }

        #[test]
        fn positive_reuse_diamond_balanced_both_branches() {
            // axis-2 × CFG diamond: Reuse at entry, both branches dec.
            let entry = jump_block(0, vec![ArcInstr::BurdenInc { var: v(0) }], b(1));
            let branch = branch_block(1, b(2), b(3));
            let then_b = jump_block(2, vec![ArcInstr::BurdenDec { var: v(0) }], b(4));
            let else_b = jump_block(3, vec![ArcInstr::BurdenDec { var: v(0) }], b(4));
            let merge = return_block(4, Vec::new());
            let func = make_burden_func(vec![entry, branch, then_b, else_b, merge]);
            let errors = verify_burden_balance(&func);
            assert!(
                errors.is_empty(),
                "Reuse + balanced-both-branch Dec must pass diamond; got: {errors:?}"
            );
        }

        #[test]
        fn negative_reuse_diamond_unbalanced_else_arm_fails() {
            // axis-2 × CFG asymmetric: only else-arm decs → merge disagrees.
            let entry = jump_block(0, vec![ArcInstr::BurdenInc { var: v(0) }], b(1));
            let branch = branch_block(1, b(2), b(3));
            let then_b = jump_block(2, Vec::new(), b(4));
            let else_b = jump_block(3, vec![ArcInstr::BurdenDec { var: v(0) }], b(4));
            let merge = return_block(4, Vec::new());
            let func = make_burden_func(vec![entry, branch, then_b, else_b, merge]);
            let errors = verify_burden_balance(&func);
            assert_eq!(
                errors.len(),
                1,
                "Reuse + asymmetric Dec (else-only) must fail predecessor agreement; \
                got: {errors:?}"
            );
        }
    }

    // axis-1 CollectionReuse slice.
    // Per `TF-9a` — CollectionReuse emits FRESH(
    // CollectionBuffer). Self-contained: LLVM emitter calls
    // ori_list_reset_buffer (uniqueness checked internally). Burden lower
    // emits BurdenInc on dst; symmetric BurdenDec mirrors §8 RL-2.
    mod collection_reuse {
        use super::super::*;

        #[test]
        fn positive_collection_reuse_scope_exit_dec_balances() {
            // axis-2 = scope-exit-only; CollectionReuse → BurdenInc; scope-
            // exit Dec balances.
            let entry = jump_block(0, vec![ArcInstr::BurdenInc { var: v(0) }], b(1));
            let exit = return_block(1, vec![ArcInstr::BurdenDec { var: v(0) }]);
            let func = make_burden_func(vec![entry, exit]);
            let errors = verify_burden_balance(&func);
            assert!(
                errors.is_empty(),
                "CollectionReuse + Dec must balance; got: {errors:?}"
            );
        }

        #[test]
        fn negative_collection_reuse_missing_dec_fails() {
            // CollectionReuse emits Inc; missing Dec → net=1.
            let entry = return_block(0, vec![ArcInstr::BurdenInc { var: v(0) }]);
            let func = make_burden_func(vec![entry]);
            let errors = verify_burden_balance(&func);
            assert_eq!(
                errors.len(),
                1,
                "CollectionReuse missing Dec must fail; got: {errors:?}"
            );
            let BurdenBalanceError {
                var,
                observed_net,
                expected_net,
                ..
            } = errors[0].clone();
            assert_eq!((var, observed_net, expected_net), (v(0), 1, 0));
        }

        #[test]
        fn positive_collection_reuse_returned_passthrough_balances() {
            // axis-2 = Owned-return-passthrough; fresh collection returned
            // to caller. Intra-block Dec at return preserves intraprocedural
            // VF-1 per §8 RL-2 terminator-transfer.
            let entry = return_block(
                0,
                vec![
                    ArcInstr::BurdenInc { var: v(0) },
                    ArcInstr::BurdenDec { var: v(0) },
                ],
            );
            let func = make_burden_func(vec![entry]);
            let errors = verify_burden_balance(&func);
            assert!(
                errors.is_empty(),
                "CollectionReuse + return-passthrough Dec must balance; got: {errors:?}"
            );
        }

        #[test]
        fn positive_collection_reuse_owned_arg_consumer_dec_at_call() {
            // axis-2 = Owned-arg consumer; fresh collection passed to Apply
            // expecting Owned param. Burden lower emits Dec at consuming
            // Apply site per §8 RL-2 ownership-transfer.
            let entry = jump_block(
                0,
                vec![
                    ArcInstr::BurdenInc { var: v(0) },
                    ArcInstr::BurdenDec { var: v(0) },
                ],
                b(1),
            );
            let exit = return_block(1, Vec::new());
            let func = make_burden_func(vec![entry, exit]);
            let errors = verify_burden_balance(&func);
            assert!(
                errors.is_empty(),
                "CollectionReuse + Owned-arg consumer Dec must balance; got: {errors:?}"
            );
        }

        #[test]
        fn positive_collection_reuse_partial_dec_balances() {
            // axis-1 × axis-2: CollectionReuse + BurdenDecPartial balances
            // whole-var grain per burden_balance.rs module doc.
            let entry = jump_block(0, vec![ArcInstr::BurdenInc { var: v(0) }], b(1));
            let exit = return_block(
                1,
                vec![ArcInstr::BurdenDecPartial {
                    var: v(0),
                    skip_fields: Vec::new(),
                }],
            );
            let func = make_burden_func(vec![entry, exit]);
            let errors = verify_burden_balance(&func);
            assert!(
                errors.is_empty(),
                "CollectionReuse + DecPartial must balance whole-var; got: {errors:?}"
            );
        }

        #[test]
        fn negative_collection_reuse_double_inc_single_dec_fails() {
            // axis-3 = Many cardinality: two CollectionReuse sites both
            // targeting v(0) → two Incs; one Dec → net=1.
            let entry = jump_block(
                0,
                vec![
                    ArcInstr::BurdenInc { var: v(0) },
                    ArcInstr::BurdenInc { var: v(0) },
                ],
                b(1),
            );
            let exit = return_block(1, vec![ArcInstr::BurdenDec { var: v(0) }]);
            let func = make_burden_func(vec![entry, exit]);
            let errors = verify_burden_balance(&func);
            assert_eq!(
                errors.len(),
                1,
                "Two CollectionReuse Incs + one Dec must surface net=1; got: {errors:?}"
            );
            let BurdenBalanceError {
                var,
                observed_net,
                expected_net,
                ..
            } = errors[0].clone();
            assert_eq!((var, observed_net, expected_net), (v(0), 1, 0));
        }

        #[test]
        fn negative_collection_reuse_diamond_only_else_arm_fails() {
            // axis-2 × CFG asymmetric: only else-arm decs → predecessor
            // disagreement.
            let entry = jump_block(0, vec![ArcInstr::BurdenInc { var: v(0) }], b(1));
            let branch = branch_block(1, b(2), b(3));
            let then_b = jump_block(2, Vec::new(), b(4));
            let else_b = jump_block(3, vec![ArcInstr::BurdenDec { var: v(0) }], b(4));
            let merge = return_block(4, Vec::new());
            let func = make_burden_func(vec![entry, branch, then_b, else_b, merge]);
            let errors = verify_burden_balance(&func);
            assert_eq!(
                errors.len(),
                1,
                "CollectionReuse + asymmetric Dec (else-only) must fail predecessor agreement; \
                got: {errors:?}"
            );
        }
    }
}
