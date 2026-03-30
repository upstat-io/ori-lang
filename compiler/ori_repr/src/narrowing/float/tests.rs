//! Tests for float precision analysis, float field summary collection,
//! and float field narrowing.

#![expect(
    clippy::expect_used,
    reason = "tests use expect() for concise assertions"
)]

use ori_ir::Name;
use ori_types::{Idx, Pool};

use crate::plan::{
    DecisionReason, DecisionSource, NarrowingPolicy, ReprAttribute, ReprDecision, ReprPlan,
};
use crate::repr::{FloatWidth, IntWidth, MachineRepr};
use crate::struct_repr::{FieldRepr, StructRepr};

use super::{
    collect_float_field_summaries, is_f32_exact, narrow_float_fields, FloatFieldSummaryTable,
    FloatRange,
};

// ── is_f32_exact ────────────────────────────────────────────────────

#[test]
fn test_f32_exact_zero() {
    assert!(is_f32_exact(0.0));
}

#[test]
fn test_f32_exact_one() {
    assert!(is_f32_exact(1.0));
}

#[test]
fn test_f32_exact_half() {
    assert!(is_f32_exact(0.5));
}

#[test]
fn test_f32_exact_hundred() {
    assert!(is_f32_exact(100.0));
}

#[test]
fn test_f32_exact_negative_zero() {
    assert!(is_f32_exact(-0.0));
}

#[test]
fn test_f32_exact_infinity() {
    assert!(is_f32_exact(f64::INFINITY));
}

#[test]
fn test_f32_exact_neg_infinity() {
    assert!(is_f32_exact(f64::NEG_INFINITY));
}

#[test]
fn test_f32_exact_f32_max() {
    assert!(is_f32_exact(f64::from(f32::MAX)));
}

#[test]
fn test_f32_exact_f32_min() {
    assert!(is_f32_exact(f64::from(f32::MIN)));
}

#[test]
fn test_f32_exact_f32_min_positive() {
    // f32's smallest positive normal value IS f32-exact.
    assert!(is_f32_exact(f64::from(f32::MIN_POSITIVE)));
}

#[test]
fn test_f32_exact_last_exact_integer() {
    // 2^24 = 16777216 is the last integer exactly representable in f32.
    assert!(is_f32_exact(16_777_216.0));
}

#[test]
fn test_not_f32_exact_one_third() {
    assert!(!is_f32_exact(1.0 / 3.0));
}

#[test]
fn test_not_f32_exact_point_one() {
    // 0.1 is NOT exactly representable in f32 (different rounding).
    assert!(!is_f32_exact(0.1));
}

#[test]
fn test_not_f32_exact_large_value() {
    assert!(!is_f32_exact(1e300));
}

#[test]
fn test_not_f32_exact_nan() {
    assert!(!is_f32_exact(f64::NAN));
}

#[test]
fn test_not_f32_exact_f64_max() {
    assert!(!is_f32_exact(f64::MAX));
}

#[test]
fn test_not_f32_exact_past_last_integer() {
    // 2^24 + 1 = 16777217 is NOT f32-exact.
    assert!(!is_f32_exact(16_777_217.0));
}

#[test]
fn test_not_f32_exact_f64_min_positive() {
    // f64's smallest positive normal is FAR below f32's subnormal range.
    assert!(!is_f32_exact(f64::MIN_POSITIVE));
}

#[test]
fn test_not_f32_exact_deep_subnormal() {
    // 1e-45 is approximately the smallest f32 subnormal.
    // The f64→f32→f64 roundtrip loses precision for this value.
    let val = 1e-45_f64;
    // is_f32_exact already handles this case correctly via roundtrip check.
    // Just verify our function agrees with the manual roundtrip.
    assert!(!is_f32_exact(val));
}

// ── FloatRange lattice ──────────────────────────────────────────────

#[test]
fn test_join_bottom_bottom() {
    assert_eq!(
        FloatRange::Bottom.join(FloatRange::Bottom),
        FloatRange::Bottom
    );
}

#[test]
fn test_join_bottom_f32exact() {
    assert_eq!(
        FloatRange::Bottom.join(FloatRange::F32Exact),
        FloatRange::F32Exact
    );
}

#[test]
fn test_join_f32exact_bottom() {
    assert_eq!(
        FloatRange::F32Exact.join(FloatRange::Bottom),
        FloatRange::F32Exact
    );
}

#[test]
fn test_join_bottom_top() {
    assert_eq!(FloatRange::Bottom.join(FloatRange::Top), FloatRange::Top);
}

#[test]
fn test_join_f32exact_f32exact() {
    assert_eq!(
        FloatRange::F32Exact.join(FloatRange::F32Exact),
        FloatRange::F32Exact
    );
}

#[test]
fn test_join_f32exact_top() {
    assert_eq!(FloatRange::F32Exact.join(FloatRange::Top), FloatRange::Top);
}

#[test]
fn test_join_top_f32exact() {
    assert_eq!(FloatRange::Top.join(FloatRange::F32Exact), FloatRange::Top);
}

#[test]
fn test_join_top_top() {
    assert_eq!(FloatRange::Top.join(FloatRange::Top), FloatRange::Top);
}

// ── FloatRange::observe ─────────────────────────────────────────────

#[test]
fn test_observe_f32_exact_value() {
    assert_eq!(FloatRange::Bottom.observe(0.5), FloatRange::F32Exact);
}

#[test]
fn test_observe_non_f32_exact_value() {
    assert_eq!(FloatRange::Bottom.observe(0.1), FloatRange::Top);
}

#[test]
fn test_observe_f32exact_then_non_exact() {
    let r = FloatRange::F32Exact.observe(0.1);
    assert_eq!(r, FloatRange::Top);
}

#[test]
fn test_observe_top_then_exact() {
    // Top absorbs everything.
    let r = FloatRange::Top.observe(0.5);
    assert_eq!(r, FloatRange::Top);
}

#[test]
fn test_observe_multiple_f32_exact() {
    let r = FloatRange::Bottom
        .observe(0.0)
        .observe(0.5)
        .observe(1.0)
        .observe(100.0);
    assert_eq!(r, FloatRange::F32Exact);
}

#[test]
fn test_observe_mixed_values() {
    let r = FloatRange::Bottom.observe(0.0).observe(0.5).observe(0.1); // 0.1 is not f32-exact
    assert_eq!(r, FloatRange::Top);
}

// ── FloatRange::observe_arithmetic ──────────────────────────────────

#[test]
fn test_observe_arithmetic_from_bottom() {
    assert_eq!(FloatRange::Bottom.observe_arithmetic(), FloatRange::Top);
}

#[test]
fn test_observe_arithmetic_from_f32exact() {
    assert_eq!(FloatRange::F32Exact.observe_arithmetic(), FloatRange::Top);
}

#[test]
fn test_observe_arithmetic_from_top() {
    assert_eq!(FloatRange::Top.observe_arithmetic(), FloatRange::Top);
}

// ── FloatRange::can_narrow_to_f32 ───────────────────────────────────

#[test]
fn test_can_narrow_f32exact() {
    assert!(FloatRange::F32Exact.can_narrow_to_f32());
}

#[test]
fn test_cannot_narrow_top() {
    assert!(!FloatRange::Top.can_narrow_to_f32());
}

#[test]
fn test_cannot_narrow_bottom() {
    // No observations → no evidence to narrow. Conservative: don't narrow.
    assert!(!FloatRange::Bottom.can_narrow_to_f32());
}

// ── FloatFieldSummaryTable ───────────────────────────────────────────

#[test]
fn test_table_empty_returns_bottom() {
    let table = FloatFieldSummaryTable::new();
    assert_eq!(table.get(Idx::INT, 0), FloatRange::Bottom);
}

#[test]
fn test_table_single_f32_exact_observation() {
    let mut table = FloatFieldSummaryTable::new();
    table.observe(Idx::INT, 0, FloatRange::F32Exact);
    assert_eq!(table.get(Idx::INT, 0), FloatRange::F32Exact);
}

#[test]
fn test_table_single_top_observation() {
    let mut table = FloatFieldSummaryTable::new();
    table.observe(Idx::INT, 0, FloatRange::Top);
    assert_eq!(table.get(Idx::INT, 0), FloatRange::Top);
}

#[test]
fn test_table_two_f32_exact_observations_join() {
    let mut table = FloatFieldSummaryTable::new();
    table.observe(Idx::INT, 0, FloatRange::F32Exact);
    table.observe(Idx::INT, 0, FloatRange::F32Exact);
    assert_eq!(table.get(Idx::INT, 0), FloatRange::F32Exact);
}

#[test]
fn test_table_f32_exact_then_top_joins_to_top() {
    let mut table = FloatFieldSummaryTable::new();
    table.observe(Idx::INT, 0, FloatRange::F32Exact);
    table.observe(Idx::INT, 0, FloatRange::Top);
    assert_eq!(table.get(Idx::INT, 0), FloatRange::Top);
}

#[test]
fn test_table_different_fields_independent() {
    let mut table = FloatFieldSummaryTable::new();
    table.observe(Idx::INT, 0, FloatRange::F32Exact);
    table.observe(Idx::INT, 1, FloatRange::Top);
    assert_eq!(table.get(Idx::INT, 0), FloatRange::F32Exact);
    assert_eq!(table.get(Idx::INT, 1), FloatRange::Top);
}

#[test]
fn test_table_different_types_independent() {
    let mut table = FloatFieldSummaryTable::new();
    table.observe(Idx::INT, 0, FloatRange::F32Exact);
    table.observe(Idx::FLOAT, 0, FloatRange::Top);
    assert_eq!(table.get(Idx::INT, 0), FloatRange::F32Exact);
    assert_eq!(table.get(Idx::FLOAT, 0), FloatRange::Top);
}

#[test]
fn test_table_bottom_observation_is_identity() {
    let mut table = FloatFieldSummaryTable::new();
    table.observe(Idx::INT, 0, FloatRange::Bottom);
    // Bottom is the identity element for join. An explicit Bottom observation
    // means "no evidence" — the entry should be Bottom.
    assert_eq!(table.get(Idx::INT, 0), FloatRange::Bottom);
}

#[test]
fn test_table_no_construct_sites_all_bottom() {
    // If no Construct sites are ever scanned, the table stays empty.
    let table = FloatFieldSummaryTable::new();
    // All queries return Bottom.
    assert_eq!(table.get(Idx::INT, 0), FloatRange::Bottom);
    assert_eq!(table.get(Idx::FLOAT, 0), FloatRange::Bottom);
    assert_eq!(table.get(Idx::BOOL, 0), FloatRange::Bottom);
}

// ── narrow_float_fields ──────────────────────────────────────────────

/// Helper: create a `FieldRepr` with canonical f64 float type.
fn float_field(index: u32) -> FieldRepr {
    FieldRepr {
        name: Name::from_raw(index),
        original_index: index,
        offset: 0,
        repr: MachineRepr::Float {
            width: FloatWidth::F64,
        },
    }
}

/// Helper: create a `FieldRepr` with canonical i64 int type.
fn int_field(index: u32) -> FieldRepr {
    FieldRepr {
        name: Name::from_raw(index),
        original_index: index,
        offset: 0,
        repr: MachineRepr::Int {
            width: IntWidth::I64,
            signed: true,
        },
    }
}

/// Helper: create a `FieldRepr` with a bool type.
fn bool_field(index: u32) -> FieldRepr {
    FieldRepr {
        name: Name::from_raw(index),
        original_index: index,
        offset: 0,
        repr: MachineRepr::Bool,
    }
}

/// Helper: set up a struct type in the plan at a given `Idx`.
fn setup_struct(plan: &mut ReprPlan, idx: Idx, fields: Vec<FieldRepr>) {
    plan.set_repr(
        idx,
        ReprDecision {
            source: DecisionSource::Canonical,
            type_idx: idx,
            repr: MachineRepr::Struct(StructRepr {
                fields,
                size: 0,
                align: 0,
                trivial: true,
            }),
            reason: DecisionReason::Canonical,
        },
    );
}

#[test]
fn test_narrow_f32_exact_field() {
    let pool = Pool::default();
    let mut plan = ReprPlan::new(NarrowingPolicy::Conservative);
    let idx = Idx::from_raw(Idx::FIRST_DYNAMIC);

    setup_struct(&mut plan, idx, vec![float_field(0)]);

    let mut table = FloatFieldSummaryTable::new();
    table.observe(idx, 0, FloatRange::F32Exact);

    narrow_float_fields(&mut plan, &pool, &table);

    let repr = plan.get_repr(idx).expect("repr must exist");
    if let MachineRepr::Struct(s) = repr {
        assert_eq!(
            s.fields[0].repr,
            MachineRepr::Float {
                width: FloatWidth::F32
            },
            "f32-exact field should be narrowed to F32"
        );
    } else {
        panic!("expected Struct repr");
    }
}

#[test]
fn test_preserve_non_f32_exact_field() {
    let pool = Pool::default();
    let mut plan = ReprPlan::new(NarrowingPolicy::Conservative);
    let idx = Idx::from_raw(Idx::FIRST_DYNAMIC);

    setup_struct(&mut plan, idx, vec![float_field(0)]);

    let mut table = FloatFieldSummaryTable::new();
    table.observe(idx, 0, FloatRange::Top);

    narrow_float_fields(&mut plan, &pool, &table);

    let repr = plan.get_repr(idx).expect("repr must exist");
    if let MachineRepr::Struct(s) = repr {
        assert_eq!(
            s.fields[0].repr,
            MachineRepr::Float {
                width: FloatWidth::F64
            },
            "non-f32-exact field should stay F64"
        );
    } else {
        panic!("expected Struct repr");
    }
}

#[test]
fn test_preserve_bottom_field() {
    let pool = Pool::default();
    let mut plan = ReprPlan::new(NarrowingPolicy::Conservative);
    let idx = Idx::from_raw(Idx::FIRST_DYNAMIC);

    setup_struct(&mut plan, idx, vec![float_field(0)]);

    // No observations (Bottom) — conservative: don't narrow.
    let table = FloatFieldSummaryTable::new();

    narrow_float_fields(&mut plan, &pool, &table);

    let repr = plan.get_repr(idx).expect("repr must exist");
    if let MachineRepr::Struct(s) = repr {
        assert_eq!(
            s.fields[0].repr,
            MachineRepr::Float {
                width: FloatWidth::F64
            },
            "Bottom (no evidence) should stay F64"
        );
    } else {
        panic!("expected Struct repr");
    }
}

#[test]
fn test_skip_repr_c() {
    let pool = Pool::default();
    let mut plan = ReprPlan::new(NarrowingPolicy::Conservative);
    let idx = Idx::from_raw(Idx::FIRST_DYNAMIC);

    setup_struct(&mut plan, idx, vec![float_field(0)]);
    plan.set_repr_attr(idx, ReprAttribute::C);

    let mut table = FloatFieldSummaryTable::new();
    table.observe(idx, 0, FloatRange::F32Exact);

    narrow_float_fields(&mut plan, &pool, &table);

    let repr = plan.get_repr(idx).expect("repr must exist");
    if let MachineRepr::Struct(s) = repr {
        assert_eq!(
            s.fields[0].repr,
            MachineRepr::Float {
                width: FloatWidth::F64
            },
            "#repr(\"c\") types must not be narrowed"
        );
    } else {
        panic!("expected Struct repr");
    }
}

#[test]
fn test_skip_repr_packed() {
    let pool = Pool::default();
    let mut plan = ReprPlan::new(NarrowingPolicy::Conservative);
    let idx = Idx::from_raw(Idx::FIRST_DYNAMIC);

    setup_struct(&mut plan, idx, vec![float_field(0)]);
    plan.set_repr_attr(idx, ReprAttribute::Packed);

    let mut table = FloatFieldSummaryTable::new();
    table.observe(idx, 0, FloatRange::F32Exact);

    narrow_float_fields(&mut plan, &pool, &table);

    let repr = plan.get_repr(idx).expect("repr must exist");
    if let MachineRepr::Struct(s) = repr {
        assert_eq!(
            s.fields[0].repr,
            MachineRepr::Float {
                width: FloatWidth::F64
            },
            "#repr(\"packed\") types must not be narrowed"
        );
    } else {
        panic!("expected Struct repr");
    }
}

#[test]
fn test_skip_repr_transparent() {
    let pool = Pool::default();
    let mut plan = ReprPlan::new(NarrowingPolicy::Conservative);
    let idx = Idx::from_raw(Idx::FIRST_DYNAMIC);

    setup_struct(&mut plan, idx, vec![float_field(0)]);
    plan.set_repr_attr(idx, ReprAttribute::Transparent);

    let mut table = FloatFieldSummaryTable::new();
    table.observe(idx, 0, FloatRange::F32Exact);

    narrow_float_fields(&mut plan, &pool, &table);

    let repr = plan.get_repr(idx).expect("repr must exist");
    if let MachineRepr::Struct(s) = repr {
        assert_eq!(
            s.fields[0].repr,
            MachineRepr::Float {
                width: FloatWidth::F64
            },
            "#repr(\"transparent\") types must not be narrowed"
        );
    } else {
        panic!("expected Struct repr");
    }
}

#[test]
fn test_allow_repr_aligned() {
    // #repr("aligned", N) alone does NOT prevent narrowing.
    let pool = Pool::default();
    let mut plan = ReprPlan::new(NarrowingPolicy::Conservative);
    let idx = Idx::from_raw(Idx::FIRST_DYNAMIC);

    setup_struct(&mut plan, idx, vec![float_field(0)]);
    plan.set_repr_attr(idx, ReprAttribute::Aligned(16));

    let mut table = FloatFieldSummaryTable::new();
    table.observe(idx, 0, FloatRange::F32Exact);

    narrow_float_fields(&mut plan, &pool, &table);

    let repr = plan.get_repr(idx).expect("repr must exist");
    if let MachineRepr::Struct(s) = repr {
        assert_eq!(
            s.fields[0].repr,
            MachineRepr::Float {
                width: FloatWidth::F32
            },
            "#repr(\"aligned\") should NOT prevent narrowing"
        );
    } else {
        panic!("expected Struct repr");
    }
}

#[test]
fn test_skip_public_type() {
    let pool = Pool::default();
    let mut plan = ReprPlan::new(NarrowingPolicy::Conservative);
    let idx = Idx::from_raw(Idx::FIRST_DYNAMIC);

    setup_struct(&mut plan, idx, vec![float_field(0)]);
    plan.set_pub_type_indices(std::iter::once(idx));

    let mut table = FloatFieldSummaryTable::new();
    table.observe(idx, 0, FloatRange::F32Exact);

    narrow_float_fields(&mut plan, &pool, &table);

    let repr = plan.get_repr(idx).expect("repr must exist");
    if let MachineRepr::Struct(s) = repr {
        assert_eq!(
            s.fields[0].repr,
            MachineRepr::Float {
                width: FloatWidth::F64
            },
            "public types must not be narrowed"
        );
    } else {
        panic!("expected Struct repr");
    }
}

#[test]
fn test_skip_narrowing_disabled() {
    let pool = Pool::default();
    let mut plan = ReprPlan::new(NarrowingPolicy::Disabled);
    let idx = Idx::from_raw(Idx::FIRST_DYNAMIC);

    setup_struct(&mut plan, idx, vec![float_field(0)]);

    let mut table = FloatFieldSummaryTable::new();
    table.observe(idx, 0, FloatRange::F32Exact);

    // Disabled policy is handled by the caller (apply_float_narrowing
    // returns early). narrow_float_fields itself doesn't check policy
    // since the caller already gates on it. This test verifies the
    // function works correctly even when called directly.
    narrow_float_fields(&mut plan, &pool, &table);

    // The function still narrows because the policy check is in the caller.
    let repr = plan.get_repr(idx).expect("repr must exist");
    if let MachineRepr::Struct(s) = repr {
        assert_eq!(
            s.fields[0].repr,
            MachineRepr::Float {
                width: FloatWidth::F32
            },
        );
    } else {
        panic!("expected Struct repr");
    }
}

#[test]
fn test_mixed_float_and_non_float_fields() {
    let pool = Pool::default();
    let mut plan = ReprPlan::new(NarrowingPolicy::Conservative);
    let idx = Idx::from_raw(Idx::FIRST_DYNAMIC);

    // Struct with int(0), float(1), bool(2), float(3)
    setup_struct(
        &mut plan,
        idx,
        vec![int_field(0), float_field(1), bool_field(2), float_field(3)],
    );

    let mut table = FloatFieldSummaryTable::new();
    table.observe(idx, 1, FloatRange::F32Exact); // field 1: f32-exact
    table.observe(idx, 3, FloatRange::Top); // field 3: non-f32-exact

    narrow_float_fields(&mut plan, &pool, &table);

    let repr = plan.get_repr(idx).expect("repr must exist");
    if let MachineRepr::Struct(s) = repr {
        // Field 0 (int) — unchanged
        assert!(matches!(s.fields[0].repr, MachineRepr::Int { .. }));
        // Field 1 (float) — narrowed to F32
        assert_eq!(
            s.fields[1].repr,
            MachineRepr::Float {
                width: FloatWidth::F32
            }
        );
        // Field 2 (bool) — unchanged
        assert_eq!(s.fields[2].repr, MachineRepr::Bool);
        // Field 3 (float) — stays F64
        assert_eq!(
            s.fields[3].repr,
            MachineRepr::Float {
                width: FloatWidth::F64
            }
        );
    } else {
        panic!("expected Struct repr");
    }
}

#[test]
fn test_combined_int_and_float_narrowing() {
    // Simulate a struct where integer narrowing already narrowed int fields and
    // float narrowing narrows float fields. The combined result should have both.
    let pool = Pool::default();
    let mut plan = ReprPlan::new(NarrowingPolicy::Conservative);
    let idx = Idx::from_raw(Idx::FIRST_DYNAMIC);

    // Start with a struct where field 0 was already narrowed by integer narrowing to I8.
    let fields = vec![
        FieldRepr {
            name: Name::from_raw(0),
            original_index: 0,
            offset: 0,
            repr: MachineRepr::Int {
                width: IntWidth::I8,
                signed: true,
            },
        },
        float_field(1),
    ];
    setup_struct(&mut plan, idx, fields);

    let mut table = FloatFieldSummaryTable::new();
    table.observe(idx, 1, FloatRange::F32Exact);

    narrow_float_fields(&mut plan, &pool, &table);

    let repr = plan.get_repr(idx).expect("repr must exist");
    if let MachineRepr::Struct(s) = repr {
        // Field 0: still I8 (from integer narrowing)
        assert_eq!(
            s.fields[0].repr,
            MachineRepr::Int {
                width: IntWidth::I8,
                signed: true,
            }
        );
        // Field 1: narrowed to F32 (from float narrowing)
        assert_eq!(
            s.fields[1].repr,
            MachineRepr::Float {
                width: FloatWidth::F32
            }
        );
    } else {
        panic!("expected Struct repr");
    }
}

/// Semantic pin: float narrowing produces a different repr than canonical.
/// This test ONLY passes when float narrowing is active — if the narrowing
/// pass is removed/broken, it reverts to F64 and this test fails.
#[test]
fn test_semantic_pin_narrowed_repr_differs_from_canonical() {
    let pool = Pool::default();
    let mut plan = ReprPlan::new(NarrowingPolicy::Conservative);
    let idx = Idx::from_raw(Idx::FIRST_DYNAMIC);

    setup_struct(&mut plan, idx, vec![float_field(0)]);

    // Before narrowing: F64 (canonical)
    let before = plan.get_repr(idx).expect("repr must exist").clone();

    let mut table = FloatFieldSummaryTable::new();
    table.observe(idx, 0, FloatRange::F32Exact);
    narrow_float_fields(&mut plan, &pool, &table);

    // After narrowing: F32 (narrowed) — different from canonical
    let after = plan.get_repr(idx).expect("repr must exist").clone();
    assert_ne!(before, after, "narrowed repr must differ from canonical");

    if let MachineRepr::Struct(s) = &after {
        assert_eq!(
            s.fields[0].repr,
            MachineRepr::Float {
                width: FloatWidth::F32
            }
        );
    } else {
        panic!("expected Struct repr");
    }
}

// ── collect_float_field_summaries ───────────────────────────────────

use ori_arc::ir::{
    ArcBlock, ArcBlockId, ArcFunction, ArcInstr, ArcTerminator, ArcValue, CtorKind, LitValue,
};
use ori_arc::uniqueness::{CowAnnotations, DropHints};
use ori_arc::ArcVarId;

/// Helper: build a minimal [`ArcFunction`] from blocks and `var_types`.
fn make_arc_func(blocks: Vec<ArcBlock>, var_types: Vec<Idx>) -> ArcFunction {
    let span_vecs: Vec<Vec<Option<ori_ir::Span>>> =
        blocks.iter().map(|bl| vec![None; bl.body.len()]).collect();
    ArcFunction {
        name: Name::from_raw(1),
        params: Vec::new(),
        return_type: Idx::UNIT,
        blocks,
        entry: ArcBlockId::new(0),
        var_types,
        var_reprs: Vec::new(),
        spans: span_vecs,
        is_fbip: false,
        num_captures: 0,
        cow_annotations: CowAnnotations::default(),
        drop_hints: DropHints::default(),
        tail_calls: Vec::new(),
    }
}

/// Helper: build a single-block `ArcBlock` with given instructions.
fn make_block(id: u32, body: Vec<ArcInstr>) -> ArcBlock {
    ArcBlock {
        id: ArcBlockId::new(id),
        params: Vec::new(),
        body,
        terminator: ArcTerminator::Return {
            value: ArcVarId::new(0),
        },
    }
}

#[test]
fn test_collect_empty_functions() {
    let pool = Pool::default();
    let plan = ReprPlan::new(NarrowingPolicy::Conservative);
    let table = collect_float_field_summaries(&plan, &pool, &[]);

    // No functions → no observations → all queries return Bottom.
    assert_eq!(table.get(Idx::FLOAT, 0), FloatRange::Bottom);
    assert_eq!(table.get(Idx::INT, 0), FloatRange::Bottom);
}

#[test]
fn test_collect_f32_exact_literal() {
    // Construct a function with:
    //   v0 = 0.5 (f32-exact float literal)
    //   v1 = Struct(name) { v0 }
    // var_types: v0 = Float, v1 = some struct type
    let pool = Pool::default();
    let plan = ReprPlan::new(NarrowingPolicy::Conservative);

    let struct_idx = Idx::from_raw(Idx::FIRST_DYNAMIC);
    let body = vec![
        ArcInstr::Let {
            dst: ArcVarId::new(0),
            ty: Idx::FLOAT,
            value: ArcValue::Literal(LitValue::Float(0.5_f64.to_bits())),
        },
        ArcInstr::Construct {
            dst: ArcVarId::new(1),
            ty: struct_idx,
            ctor: CtorKind::Struct(Name::from_raw(100)),
            args: vec![ArcVarId::new(0)],
        },
    ];

    let var_types = vec![Idx::FLOAT, struct_idx];
    let func = make_arc_func(vec![make_block(0, body)], var_types);

    let table = collect_float_field_summaries(&plan, &pool, &[func]);

    assert_eq!(
        table.get(struct_idx, 0),
        FloatRange::F32Exact,
        "0.5 is f32-exact, so field 0 should be F32Exact"
    );
}

#[test]
fn test_collect_non_f32_exact_literal() {
    // Construct a function with:
    //   v0 = 0.1 (NOT f32-exact)
    //   v1 = Struct(name) { v0 }
    let pool = Pool::default();
    let plan = ReprPlan::new(NarrowingPolicy::Conservative);

    let struct_idx = Idx::from_raw(Idx::FIRST_DYNAMIC);
    let body = vec![
        ArcInstr::Let {
            dst: ArcVarId::new(0),
            ty: Idx::FLOAT,
            value: ArcValue::Literal(LitValue::Float(0.1_f64.to_bits())),
        },
        ArcInstr::Construct {
            dst: ArcVarId::new(1),
            ty: struct_idx,
            ctor: CtorKind::Struct(Name::from_raw(100)),
            args: vec![ArcVarId::new(0)],
        },
    ];

    let var_types = vec![Idx::FLOAT, struct_idx];
    let func = make_arc_func(vec![make_block(0, body)], var_types);

    let table = collect_float_field_summaries(&plan, &pool, &[func]);

    assert_eq!(
        table.get(struct_idx, 0),
        FloatRange::Top,
        "0.1 is NOT f32-exact, so field 0 should be Top"
    );
}

#[test]
fn test_collect_variable_arg_yields_top() {
    // Construct a function with:
    //   v0 = some variable (not a float literal — e.g. a Var reference)
    //   v1 = Struct(name) { v0 }
    // The float literal map won't contain v0, so the function should
    // conservatively assign Top.
    let pool = Pool::default();
    let plan = ReprPlan::new(NarrowingPolicy::Conservative);

    let struct_idx = Idx::from_raw(Idx::FIRST_DYNAMIC);
    // v0 is defined as a Var (copy of another variable, not a literal).
    // We need v2 as the "source" even though it won't be scanned as a literal.
    let body = vec![
        ArcInstr::Let {
            dst: ArcVarId::new(0),
            ty: Idx::FLOAT,
            value: ArcValue::Var(ArcVarId::new(2)),
        },
        ArcInstr::Construct {
            dst: ArcVarId::new(1),
            ty: struct_idx,
            ctor: CtorKind::Struct(Name::from_raw(100)),
            args: vec![ArcVarId::new(0)],
        },
    ];

    // v0 = float, v1 = struct, v2 = float (the "source" for the Var copy)
    let var_types = vec![Idx::FLOAT, struct_idx, Idx::FLOAT];
    let func = make_arc_func(vec![make_block(0, body)], var_types);

    let table = collect_float_field_summaries(&plan, &pool, &[func]);

    assert_eq!(
        table.get(struct_idx, 0),
        FloatRange::Top,
        "variable (non-literal) float arg should produce Top"
    );
}

#[test]
fn test_collect_mixed_float_and_non_float_fields() {
    // Struct with 3 fields: int(v0), float-f32-exact(v1), float-non-exact(v2)
    // Only float-typed fields should appear in the table.
    let pool = Pool::default();
    let plan = ReprPlan::new(NarrowingPolicy::Conservative);

    let struct_idx = Idx::from_raw(Idx::FIRST_DYNAMIC);
    let body = vec![
        // v0: int literal (not float)
        ArcInstr::Let {
            dst: ArcVarId::new(0),
            ty: Idx::INT,
            value: ArcValue::Literal(LitValue::Int(42)),
        },
        // v1: f32-exact float literal (0.5)
        ArcInstr::Let {
            dst: ArcVarId::new(1),
            ty: Idx::FLOAT,
            value: ArcValue::Literal(LitValue::Float(0.5_f64.to_bits())),
        },
        // v2: non-f32-exact float literal (0.1)
        ArcInstr::Let {
            dst: ArcVarId::new(2),
            ty: Idx::FLOAT,
            value: ArcValue::Literal(LitValue::Float(0.1_f64.to_bits())),
        },
        // v3 = Struct { v0, v1, v2 }
        ArcInstr::Construct {
            dst: ArcVarId::new(3),
            ty: struct_idx,
            ctor: CtorKind::Struct(Name::from_raw(100)),
            args: vec![ArcVarId::new(0), ArcVarId::new(1), ArcVarId::new(2)],
        },
    ];

    let var_types = vec![Idx::INT, Idx::FLOAT, Idx::FLOAT, struct_idx];
    let func = make_arc_func(vec![make_block(0, body)], var_types);

    let table = collect_float_field_summaries(&plan, &pool, &[func]);

    // Field 0 (int) — no float observation expected (Bottom).
    assert_eq!(
        table.get(struct_idx, 0),
        FloatRange::Bottom,
        "int field should have no float observation"
    );
    // Field 1 (float, 0.5) — f32-exact.
    assert_eq!(
        table.get(struct_idx, 1),
        FloatRange::F32Exact,
        "0.5 float field should be F32Exact"
    );
    // Field 2 (float, 0.1) — not f32-exact → Top.
    assert_eq!(
        table.get(struct_idx, 2),
        FloatRange::Top,
        "0.1 float field should be Top"
    );
}

#[test]
fn test_collect_multiple_construct_sites_join() {
    // Two Construct sites for the same struct type:
    //   site 1: field 0 = 0.5 (f32-exact)
    //   site 2: field 0 = 1.0 (f32-exact)
    // Joined result should be F32Exact.
    let pool = Pool::default();
    let plan = ReprPlan::new(NarrowingPolicy::Conservative);

    let struct_idx = Idx::from_raw(Idx::FIRST_DYNAMIC);
    let body = vec![
        ArcInstr::Let {
            dst: ArcVarId::new(0),
            ty: Idx::FLOAT,
            value: ArcValue::Literal(LitValue::Float(0.5_f64.to_bits())),
        },
        ArcInstr::Construct {
            dst: ArcVarId::new(1),
            ty: struct_idx,
            ctor: CtorKind::Struct(Name::from_raw(100)),
            args: vec![ArcVarId::new(0)],
        },
        ArcInstr::Let {
            dst: ArcVarId::new(2),
            ty: Idx::FLOAT,
            value: ArcValue::Literal(LitValue::Float(1.0_f64.to_bits())),
        },
        ArcInstr::Construct {
            dst: ArcVarId::new(3),
            ty: struct_idx,
            ctor: CtorKind::Struct(Name::from_raw(100)),
            args: vec![ArcVarId::new(2)],
        },
    ];

    let var_types = vec![Idx::FLOAT, struct_idx, Idx::FLOAT, struct_idx];
    let func = make_arc_func(vec![make_block(0, body)], var_types);

    let table = collect_float_field_summaries(&plan, &pool, &[func]);

    assert_eq!(
        table.get(struct_idx, 0),
        FloatRange::F32Exact,
        "two f32-exact sites should join to F32Exact"
    );
}

#[test]
fn test_collect_multiple_sites_one_non_exact_joins_to_top() {
    // Two Construct sites for the same struct type:
    //   site 1: field 0 = 0.5 (f32-exact)
    //   site 2: field 0 = 0.1 (NOT f32-exact)
    // Joined result should be Top.
    let pool = Pool::default();
    let plan = ReprPlan::new(NarrowingPolicy::Conservative);

    let struct_idx = Idx::from_raw(Idx::FIRST_DYNAMIC);
    let body = vec![
        ArcInstr::Let {
            dst: ArcVarId::new(0),
            ty: Idx::FLOAT,
            value: ArcValue::Literal(LitValue::Float(0.5_f64.to_bits())),
        },
        ArcInstr::Construct {
            dst: ArcVarId::new(1),
            ty: struct_idx,
            ctor: CtorKind::Struct(Name::from_raw(100)),
            args: vec![ArcVarId::new(0)],
        },
        ArcInstr::Let {
            dst: ArcVarId::new(2),
            ty: Idx::FLOAT,
            value: ArcValue::Literal(LitValue::Float(0.1_f64.to_bits())),
        },
        ArcInstr::Construct {
            dst: ArcVarId::new(3),
            ty: struct_idx,
            ctor: CtorKind::Struct(Name::from_raw(100)),
            args: vec![ArcVarId::new(2)],
        },
    ];

    let var_types = vec![Idx::FLOAT, struct_idx, Idx::FLOAT, struct_idx];
    let func = make_arc_func(vec![make_block(0, body)], var_types);

    let table = collect_float_field_summaries(&plan, &pool, &[func]);

    assert_eq!(
        table.get(struct_idx, 0),
        FloatRange::Top,
        "one non-f32-exact site should widen to Top"
    );
}

#[test]
fn test_collect_skips_non_struct_ctor_kinds() {
    // ListLiteral constructor should be skipped by the match in
    // collect_float_field_summaries (only Struct/Tuple/EnumVariant proceed).
    let pool = Pool::default();
    let plan = ReprPlan::new(NarrowingPolicy::Conservative);

    let list_idx = Idx::from_raw(Idx::FIRST_DYNAMIC);
    let body = vec![
        ArcInstr::Let {
            dst: ArcVarId::new(0),
            ty: Idx::FLOAT,
            value: ArcValue::Literal(LitValue::Float(0.5_f64.to_bits())),
        },
        ArcInstr::Construct {
            dst: ArcVarId::new(1),
            ty: list_idx,
            ctor: CtorKind::ListLiteral,
            args: vec![ArcVarId::new(0)],
        },
    ];

    let var_types = vec![Idx::FLOAT, list_idx];
    let func = make_arc_func(vec![make_block(0, body)], var_types);

    let table = collect_float_field_summaries(&plan, &pool, &[func]);

    assert_eq!(
        table.get(list_idx, 0),
        FloatRange::Bottom,
        "ListLiteral ctor should be skipped — no observations"
    );
}

#[test]
fn test_collect_enum_variant_ctor() {
    // EnumVariant constructor should be scanned (same as Struct).
    let pool = Pool::default();
    let plan = ReprPlan::new(NarrowingPolicy::Conservative);

    let enum_idx = Idx::from_raw(Idx::FIRST_DYNAMIC);
    let body = vec![
        ArcInstr::Let {
            dst: ArcVarId::new(0),
            ty: Idx::FLOAT,
            value: ArcValue::Literal(LitValue::Float(0.5_f64.to_bits())),
        },
        ArcInstr::Construct {
            dst: ArcVarId::new(1),
            ty: enum_idx,
            ctor: CtorKind::EnumVariant {
                enum_name: Name::from_raw(200),
                variant: 0,
            },
            args: vec![ArcVarId::new(0)],
        },
    ];

    let var_types = vec![Idx::FLOAT, enum_idx];
    let func = make_arc_func(vec![make_block(0, body)], var_types);

    let table = collect_float_field_summaries(&plan, &pool, &[func]);

    assert_eq!(
        table.get(enum_idx, 0),
        FloatRange::F32Exact,
        "EnumVariant ctor should scan float fields"
    );
}

#[test]
fn test_collect_tuple_ctor() {
    // Tuple constructor should be scanned (same as Struct).
    let pool = Pool::default();
    let plan = ReprPlan::new(NarrowingPolicy::Conservative);

    let tuple_idx = Idx::from_raw(Idx::FIRST_DYNAMIC);
    let body = vec![
        ArcInstr::Let {
            dst: ArcVarId::new(0),
            ty: Idx::FLOAT,
            value: ArcValue::Literal(LitValue::Float(0.5_f64.to_bits())),
        },
        ArcInstr::Construct {
            dst: ArcVarId::new(1),
            ty: tuple_idx,
            ctor: CtorKind::Tuple,
            args: vec![ArcVarId::new(0)],
        },
    ];

    let var_types = vec![Idx::FLOAT, tuple_idx];
    let func = make_arc_func(vec![make_block(0, body)], var_types);

    let table = collect_float_field_summaries(&plan, &pool, &[func]);

    assert_eq!(
        table.get(tuple_idx, 0),
        FloatRange::F32Exact,
        "Tuple ctor should scan float fields"
    );
}

#[test]
fn test_collect_across_multiple_functions() {
    // Two functions, each constructing the same struct type with different
    // float values. The table should join across functions.
    let pool = Pool::default();
    let plan = ReprPlan::new(NarrowingPolicy::Conservative);

    let struct_idx = Idx::from_raw(Idx::FIRST_DYNAMIC);

    // Function 1: field 0 = 0.5 (f32-exact)
    let body1 = vec![
        ArcInstr::Let {
            dst: ArcVarId::new(0),
            ty: Idx::FLOAT,
            value: ArcValue::Literal(LitValue::Float(0.5_f64.to_bits())),
        },
        ArcInstr::Construct {
            dst: ArcVarId::new(1),
            ty: struct_idx,
            ctor: CtorKind::Struct(Name::from_raw(100)),
            args: vec![ArcVarId::new(0)],
        },
    ];
    let func1 = make_arc_func(vec![make_block(0, body1)], vec![Idx::FLOAT, struct_idx]);

    // Function 2: field 0 = 0.1 (NOT f32-exact)
    let body2 = vec![
        ArcInstr::Let {
            dst: ArcVarId::new(0),
            ty: Idx::FLOAT,
            value: ArcValue::Literal(LitValue::Float(0.1_f64.to_bits())),
        },
        ArcInstr::Construct {
            dst: ArcVarId::new(1),
            ty: struct_idx,
            ctor: CtorKind::Struct(Name::from_raw(100)),
            args: vec![ArcVarId::new(0)],
        },
    ];
    let func2 = make_arc_func(vec![make_block(0, body2)], vec![Idx::FLOAT, struct_idx]);

    let table = collect_float_field_summaries(&plan, &pool, &[func1, func2]);

    assert_eq!(
        table.get(struct_idx, 0),
        FloatRange::Top,
        "cross-function join: f32-exact + non-exact → Top"
    );
}

#[test]
fn test_collect_across_multiple_blocks() {
    // Single function with two blocks, each containing a Construct site.
    let pool = Pool::default();
    let plan = ReprPlan::new(NarrowingPolicy::Conservative);

    let struct_idx = Idx::from_raw(Idx::FIRST_DYNAMIC);

    let block0 = ArcBlock {
        id: ArcBlockId::new(0),
        params: Vec::new(),
        body: vec![
            ArcInstr::Let {
                dst: ArcVarId::new(0),
                ty: Idx::FLOAT,
                value: ArcValue::Literal(LitValue::Float(0.5_f64.to_bits())),
            },
            ArcInstr::Construct {
                dst: ArcVarId::new(1),
                ty: struct_idx,
                ctor: CtorKind::Struct(Name::from_raw(100)),
                args: vec![ArcVarId::new(0)],
            },
        ],
        terminator: ArcTerminator::Jump {
            target: ArcBlockId::new(1),
            args: Vec::new(),
        },
    };

    let block1 = ArcBlock {
        id: ArcBlockId::new(1),
        params: Vec::new(),
        body: vec![
            ArcInstr::Let {
                dst: ArcVarId::new(2),
                ty: Idx::FLOAT,
                value: ArcValue::Literal(LitValue::Float(1.0_f64.to_bits())),
            },
            ArcInstr::Construct {
                dst: ArcVarId::new(3),
                ty: struct_idx,
                ctor: CtorKind::Struct(Name::from_raw(100)),
                args: vec![ArcVarId::new(2)],
            },
        ],
        terminator: ArcTerminator::Return {
            value: ArcVarId::new(3),
        },
    };

    let var_types = vec![Idx::FLOAT, struct_idx, Idx::FLOAT, struct_idx];
    let func = make_arc_func(vec![block0, block1], var_types);

    let table = collect_float_field_summaries(&plan, &pool, &[func]);

    assert_eq!(
        table.get(struct_idx, 0),
        FloatRange::F32Exact,
        "two f32-exact sites across blocks should join to F32Exact"
    );
}
