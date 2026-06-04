//! Phase-7 mechanical burden-lowering tests (probe path).
//!
//! Pins [`super::lower_burden_ops_to_rc`]: under the probe
//! (`predicate_stack_rc_disabled`), surviving whole-var `BurdenInc` /
//! `BurdenDec` lower to real `RcInc` / `RcDec`, while the field-grain
//! `BurdenDecPartial` / `BurdenDecField` / `BurdenDecVariant` variants are
//! left intact for codegen's per-field / per-variant drop glue.
//!
//! RC counts use the SSOT `crate::pipeline::rc_count::count_rc_ops`.

use super::{
    compute_elidable_fresh_self_alloc_incs, compute_lineage_alloc_aware_net, lower_burden_ops_to_rc,
};
use crate::ir::{
    ArcBlock, ArcBlockId, ArcFunction, ArcInstr, ArcTerminator, ArcValue, ArcVarId, CtorKind,
    PrimOp, ValueRepr,
};
use crate::pipeline::rc_count::count_rc_ops;
use ori_ir::BinaryOp;
use ori_types::{Idx, Pool};
use rustc_hash::FxHashMap;

fn v(n: u32) -> ArcVarId {
    ArcVarId::new(n)
}

/// Single-block function with `body`, `num_vars` typed slots all `RcPointer`
/// (heap-backed → `RcStrategy::HeapPointer` under the default `Pool`), and a
/// `Return` of var 0. Mirrors the burden-walk precondition that whole-var
/// burden ops only target RC-bearing (non-scalar) vars.
fn rc_pointer_func(num_vars: u32, body: Vec<ArcInstr>) -> ArcFunction {
    let var_types: Vec<Idx> = (0..num_vars).map(|_| Idx::from_raw(0)).collect();
    let var_reprs: Vec<ValueRepr> = (0..num_vars).map(|_| ValueRepr::RcPointer).collect();
    ArcFunction {
        var_types,
        var_reprs,
        blocks: vec![ArcBlock {
            id: ArcBlockId::new(0),
            params: Vec::new(),
            body,
            terminator: ArcTerminator::Return { value: v(0) },
        }],
        ..Default::default()
    }
}

/// Count burden ops of every kind remaining in `func`.
fn burden_count(func: &ArcFunction) -> usize {
    func.blocks
        .iter()
        .flat_map(|b| &b.body)
        .filter(|i| {
            matches!(
                i,
                ArcInstr::BurdenInc { .. }
                    | ArcInstr::BurdenDec { .. }
                    | ArcInstr::BurdenDecPartial { .. }
                    | ArcInstr::BurdenDecField { .. }
                    | ArcInstr::BurdenDecVariant { .. }
            )
        })
        .count()
}

#[test]
fn lower_whole_var_burden_inc_dec_becomes_real_rc() {
    let pool = Pool::default();
    let mut func = rc_pointer_func(
        1,
        vec![
            ArcInstr::BurdenInc { var: v(0) },
            ArcInstr::BurdenDec { var: v(0) },
        ],
    );

    // Semantic pin (a): pre-lowering there are zero real RC ops — only burden
    // markers exist.
    let before = count_rc_ops(&func);
    assert_eq!(before.inc, 0, "no real RcInc before lowering");
    assert_eq!(before.dec, 0, "no real RcDec before lowering");

    lower_burden_ops_to_rc(&mut func, &pool, &rustc_hash::FxHashSet::default());

    // Semantic pin (b): the burden path produced real, balanced RC ops.
    let after = count_rc_ops(&func);
    assert_eq!(after.inc, 1, "BurdenInc lowered to exactly one real RcInc");
    assert_eq!(after.dec, 1, "BurdenDec lowered to exactly one real RcDec");
    // Negative pin: no whole-var burden marker survives the lowering.
    assert_eq!(
        burden_count(&func),
        0,
        "whole-var burden ops fully consumed by Phase-7 lowering"
    );
}

#[test]
fn lower_preserves_field_grain_burden_variants() {
    let pool = Pool::default();
    // BurdenDecPartial / BurdenDecField / BurdenDecVariant carry field/variant
    // info codegen consumes directly (instr_dispatch.rs); Phase-7 lowering must
    // NOT rewrite them to a whole-var RcDec (that would double-drop the
    // moved-out / surviving fields).
    let mut func = rc_pointer_func(
        2,
        vec![
            ArcInstr::BurdenInc { var: v(0) },
            ArcInstr::BurdenDecPartial {
                var: v(0),
                skip_fields: vec![0],
            },
            ArcInstr::BurdenDecField {
                base: v(1),
                field: 0,
            },
            ArcInstr::BurdenDecVariant { var: v(1) },
        ],
    );

    lower_burden_ops_to_rc(&mut func, &pool, &rustc_hash::FxHashSet::default());

    // The whole-var BurdenInc lowered; the three field-grain variants survive
    // verbatim for codegen.
    let body = &func.blocks[0].body;
    assert!(
        matches!(body[0], ArcInstr::RcInc { var, count: 1, .. } if var == v(0)),
        "whole-var BurdenInc lowered to RcInc"
    );
    assert!(
        matches!(&body[1], ArcInstr::BurdenDecPartial { var, skip_fields } if *var == v(0) && skip_fields == &[0]),
        "BurdenDecPartial preserved verbatim — NOT rewritten to RcDec"
    );
    assert!(
        matches!(body[2], ArcInstr::BurdenDecField { base, field: 0 } if base == v(1)),
        "BurdenDecField preserved verbatim"
    );
    assert!(
        matches!(body[3], ArcInstr::BurdenDecVariant { var } if var == v(1)),
        "BurdenDecVariant preserved verbatim"
    );

    // Negative pin: the three field-grain variants still count as burden ops.
    assert_eq!(
        burden_count(&func),
        3,
        "field-grain burden variants are NOT consumed by whole-var lowering"
    );
    // Negative pin: lowering whole-var ops to a whole-var RcDec for v(1) would
    // be a double-drop — assert NO whole-var RcDec was synthesized for v(1).
    let v1_whole_rcdec = func
        .blocks
        .iter()
        .flat_map(|b| &b.body)
        .filter(|i| matches!(i, ArcInstr::RcDec { var, .. } if *var == v(1)))
        .count();
    assert_eq!(
        v1_whole_rcdec, 0,
        "no spurious whole-var RcDec synthesized for a field-grain-only var"
    );
}

#[test]
fn lower_leaves_scalar_repr_burden_in_place() {
    let pool = Pool::default();
    // Scalars never carry RcStrategy. emit_burden_ops filters them, but the
    // RE-2 backstop must leave a (contract-violating) scalar burden op in place
    // rather than synthesize unsound RC.
    let mut func = rc_pointer_func(1, vec![ArcInstr::BurdenInc { var: v(0) }]);
    func.var_reprs[0] = ValueRepr::Scalar;

    lower_burden_ops_to_rc(&mut func, &pool, &rustc_hash::FxHashSet::default());

    let after = count_rc_ops(&func);
    assert_eq!(
        after.inc, 0,
        "no RcInc synthesized for a scalar-repr burden op"
    );
    assert_eq!(
        burden_count(&func),
        1,
        "scalar-repr burden op left in place (codegen no-ops it)"
    );
}

// --- M3 + Phase-7 elision pins (broad-shape collection-source freeing) ---

/// Single-block reps map where each var is its own rep (no same-alloc aliasing).
fn identity_reps(n: u32) -> FxHashMap<ArcVarId, ArcVarId> {
    (0..n).map(|i| (v(i), v(i))).collect()
}

fn list_construct(dst: ArcVarId, args: Vec<ArcVarId>) -> ArcInstr {
    ArcInstr::Construct {
        dst,
        ty: Idx::from_raw(0),
        ctor: CtorKind::ListLiteral,
        args,
    }
}

/// Semantic pin: a single-reference FRESH self-alloc whose lineage is read-only
/// (no COW-mutation operand) has alloc-aware net == 1 (alloc +1, fresh inc +1,
/// one dec −1 → counting alloc, the fresh inc is the surplus), so its fresh inc
/// is ELIDABLE — eliding restores the alloc-aware balance to 0, freeing the
/// value exactly once. Mirrors `coll_list_index` (the redundant-inc leak).
#[test]
fn elide_redundant_fresh_inc_for_read_only_self_alloc() {
    // %0 = Construct List()  (fresh self-alloc, +1)
    // burden_inc %0          (paired fresh inc, +1)
    // burden_dec %0          (release, −1)  → alloc-aware net = 1
    let func = rc_pointer_func(
        1,
        vec![
            list_construct(v(0), Vec::new()),
            ArcInstr::BurdenInc { var: v(0) },
            ArcInstr::BurdenDec { var: v(0) },
        ],
    );
    let reps = identity_reps(1);
    let net = compute_lineage_alloc_aware_net(&func, &reps);
    assert_eq!(
        net.get(&v(0)).copied(),
        Some(1),
        "read-only single-ref self-alloc lineage nets +1 (redundant fresh inc surplus)"
    );
    let elidable =
        compute_elidable_fresh_self_alloc_incs(&func, &reps, &ori_ir::StringInterner::new());
    assert!(
        elidable.contains(&v(0)),
        "redundant fresh inc of a read-only single-ref self-alloc is elidable"
    );
}

/// Negative pin: a FRESH self-alloc whose lineage flows into a COW-mutation
/// operand (a `PrimOp Binary` with an `RcPtr` operand — list `+`/concat) KEEPS its
/// fresh inc — the COW helper reads the runtime refcount to choose
/// copy-vs-mutate, so the fresh inc is LOAD-BEARING (raises rc ≥ 2 → copy).
/// Eliding it would mutate the shared value in place. Mirrors
/// `coll_list_cow_concat_shared`.
#[test]
fn keep_fresh_inc_for_cow_mutated_self_alloc() {
    // %0 = Construct List()       (fresh self-alloc xs)
    // burden_inc %0
    // %1 = Construct List()       (rhs)
    // burden_inc %1
    // %2 = %0 + %1                (COW concat — reads %0's refcount)
    // burden_dec %0 ; burden_dec %1
    let func = rc_pointer_func(
        3,
        vec![
            list_construct(v(0), Vec::new()),
            ArcInstr::BurdenInc { var: v(0) },
            list_construct(v(1), Vec::new()),
            ArcInstr::BurdenInc { var: v(1) },
            ArcInstr::Let {
                dst: v(2),
                ty: Idx::from_raw(0),
                value: ArcValue::PrimOp {
                    op: PrimOp::Binary(BinaryOp::Add),
                    args: vec![v(0), v(1)],
                },
            },
            ArcInstr::BurdenDec { var: v(0) },
            ArcInstr::BurdenDec { var: v(1) },
        ],
    );
    let reps = identity_reps(3);
    let elidable =
        compute_elidable_fresh_self_alloc_incs(&func, &reps, &ori_ir::StringInterner::new());
    assert!(
        !elidable.contains(&v(0)),
        "COW-mutated self-alloc (list + operand) keeps its load-bearing fresh inc"
    );
    assert!(
        !elidable.contains(&v(1)),
        "the other COW `+` operand also keeps its fresh inc"
    );
}

/// Negative pin (double-free protection): a FRESH self-alloc with a move-alias
/// dec but no paired dup inc has alloc-aware net == 0 (alloc +1, fresh inc +1,
/// TWO decs −2 → the fresh inc balances the move-alias's unpaired dec). Net != 1
/// → the fresh inc is NOT elidable: eliding would net −1 = a double-free.
/// Mirrors `coll_list_length_one`.
#[test]
fn keep_fresh_inc_when_net_not_one_move_alias_dec() {
    // %0 = Construct List()  (fresh self-alloc)
    // burden_inc %0          (+1)
    // burden_dec %0          (−1)  ← the move-alias %1=%0 dec, attributed to rep %0
    // burden_dec %0          (−1)  ← the fresh value's own release
    // net counting alloc = 1 + 1 − 2 = 0 (NOT 1) → keep.
    let func = rc_pointer_func(
        1,
        vec![
            list_construct(v(0), Vec::new()),
            ArcInstr::BurdenInc { var: v(0) },
            ArcInstr::BurdenDec { var: v(0) },
            ArcInstr::BurdenDec { var: v(0) },
        ],
    );
    let reps = identity_reps(1);
    let net = compute_lineage_alloc_aware_net(&func, &reps);
    assert_eq!(
        net.get(&v(0)).copied(),
        Some(0),
        "move-alias-dec lineage nets 0 (fresh inc balances the unpaired move dec)"
    );
    let elidable =
        compute_elidable_fresh_self_alloc_incs(&func, &reps, &ori_ir::StringInterner::new());
    assert!(
        !elidable.contains(&v(0)),
        "net != 1 → fresh inc kept (eliding would double-free, net −1)"
    );
}
