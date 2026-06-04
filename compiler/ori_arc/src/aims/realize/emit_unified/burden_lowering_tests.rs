//! Phase-7 mechanical burden-lowering tests (probe path).
//!
//! Pins [`super::lower_burden_ops_to_rc`]: under the probe
//! (`predicate_stack_rc_disabled`), surviving whole-var `BurdenInc` /
//! `BurdenDec` lower to real `RcInc` / `RcDec`, while the field-grain
//! `BurdenDecPartial` / `BurdenDecField` / `BurdenDecVariant` variants are
//! left intact for codegen's per-field / per-variant drop glue.
//!
//! RC counts use the SSOT `crate::pipeline::rc_count::count_rc_ops`.

use super::lower_burden_ops_to_rc;
use crate::ir::{ArcBlock, ArcBlockId, ArcFunction, ArcInstr, ArcTerminator, ArcVarId, ValueRepr};
use crate::pipeline::rc_count::count_rc_ops;
use ori_types::{Idx, Pool};

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

    lower_burden_ops_to_rc(&mut func, &pool);

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

    lower_burden_ops_to_rc(&mut func, &pool);

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

    lower_burden_ops_to_rc(&mut func, &pool);

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
