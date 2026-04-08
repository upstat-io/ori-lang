//! Tests for integer narrowing (§04.1–§04.2, §04.4 Phase C).
//!
//! - Phase A tests (§04.1): struct/tuple field narrowing
//! - ABI boundary tests (§04.2): boundary classification, widening policy
//! - Phase C tests (§04.4): collection element narrowing

use ori_ir::Name;
use ori_types::{Idx, Pool};

use ori_ir::BinaryOp;

use crate::narrowing::abi::{
    self, AbiBoundary, CrossModuleAgreement, FunctionBoundaryInfo, WidthRequirement,
};
use crate::narrowing::int::{narrow_collection_elements, narrow_struct_fields};
use crate::narrowing::overflow::{self, OverflowStrategy};
use crate::plan::{
    DecisionReason, DecisionSource, NarrowingPolicy, ReprAttribute, ReprDecision, ReprPlan,
};
use crate::range::field_summary::{update_element_summaries, ElementSummaryTable};
use crate::range::ValueRange;
use crate::repr::{IntWidth, MachineRepr};
use crate::struct_repr::{FatRepr, FieldRepr, StructRepr, TupleRepr};

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

/// Helper: create a `FieldRepr` with a specific int width.
fn int_field_width(index: u32, width: IntWidth) -> FieldRepr {
    FieldRepr {
        name: Name::from_raw(index),
        original_index: index,
        offset: 0,
        repr: MachineRepr::Int {
            width,
            signed: true,
        },
    }
}

/// Helper: create a `FieldRepr` with a bool type (non-int field).
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

/// Helper: set up a tuple type in the plan at a given `Idx`.
fn setup_tuple(plan: &mut ReprPlan, idx: Idx, elements: Vec<FieldRepr>) {
    plan.set_repr(
        idx,
        ReprDecision {
            source: DecisionSource::Canonical,
            type_idx: idx,
            repr: MachineRepr::Tuple(TupleRepr {
                elements,
                size: 0,
                align: 0,
                trivial: true,
            }),
            reason: DecisionReason::Canonical,
        },
    );
}

/// Helper: get the int width of a struct field after narrowing.
fn struct_field_width(plan: &ReprPlan, idx: Idx, field_index: usize) -> Option<IntWidth> {
    match plan.get_repr(idx)? {
        MachineRepr::Struct(s) => match &s.fields[field_index].repr {
            MachineRepr::Int { width, .. } => Some(*width),
            _ => None,
        },
        _ => None,
    }
}

/// Helper: get the int width of a tuple element after narrowing.
fn tuple_element_width(plan: &ReprPlan, idx: Idx, element_index: usize) -> Option<IntWidth> {
    match plan.get_repr(idx)? {
        MachineRepr::Tuple(t) => match &t.elements[element_index].repr {
            MachineRepr::Int { width, .. } => Some(*width),
            _ => None,
        },
        _ => None,
    }
}

// ────────────────────────────────────────────────────
// Semantic pin: Pixel { r, g, b, a: int } with 0..255 fields → i8
// This test ONLY passes with integer narrowing enabled.
// ────────────────────────────────────────────────────

#[test]
fn semantic_pin_pixel_struct_narrows_to_i8() {
    let pool = Pool::new();
    let mut plan = ReprPlan::new(NarrowingPolicy::Aggressive);

    let idx = Idx::from_raw(Idx::FIRST_DYNAMIC);
    setup_struct(
        &mut plan,
        idx,
        vec![int_field(0), int_field(1), int_field(2), int_field(3)],
    );

    // Simulate §03 field-range results: all fields are [0, 255].
    for field in 0u32..4 {
        plan.join_field_range(idx, field, ValueRange::Bounded { lo: 0, hi: 255 });
    }

    narrow_struct_fields(&mut plan, &pool);

    // 0..255 does NOT fit in signed i8 (max 127). min_width() returns I16.
    for field in 0usize..4 {
        assert_eq!(
            struct_field_width(&plan, idx, field),
            Some(IntWidth::I16),
            "Pixel field {field} should narrow to i16 (0..255 exceeds signed i8 max 127)"
        );
    }
}

// ────────────────────────────────────────────────────
// Semantic pin: Pixel with [-128, 127] fields → i8
// ────────────────────────────────────────────────────

#[test]
fn semantic_pin_pixel_signed_range_narrows_to_i8() {
    let pool = Pool::new();
    let mut plan = ReprPlan::new(NarrowingPolicy::Aggressive);

    let idx = Idx::from_raw(Idx::FIRST_DYNAMIC);
    setup_struct(
        &mut plan,
        idx,
        vec![int_field(0), int_field(1), int_field(2), int_field(3)],
    );

    for field in 0u32..4 {
        plan.join_field_range(idx, field, ValueRange::Bounded { lo: -128, hi: 127 });
    }

    narrow_struct_fields(&mut plan, &pool);

    for field in 0usize..4 {
        assert_eq!(
            struct_field_width(&plan, idx, field),
            Some(IntWidth::I8),
            "Pixel field {field} should narrow to i8 ([-128, 127] fits in signed i8)"
        );
    }
}

// ────────────────────────────────────────────────────
// Boundary: field range [0, 255] → i16 (NOT i8, since signed i8 max = 127)
// ────────────────────────────────────────────────────

#[test]
fn boundary_unsigned_byte_range_narrows_to_i16() {
    let pool = Pool::new();
    let mut plan = ReprPlan::new(NarrowingPolicy::Aggressive);

    let idx = Idx::from_raw(Idx::FIRST_DYNAMIC);
    setup_struct(&mut plan, idx, vec![int_field(0)]);
    plan.join_field_range(idx, 0, ValueRange::Bounded { lo: 0, hi: 255 });

    narrow_struct_fields(&mut plan, &pool);

    assert_eq!(
        struct_field_width(&plan, idx, 0),
        Some(IntWidth::I16),
        "[0, 255] exceeds signed i8 max (127) → i16"
    );
}

// ────────────────────────────────────────────────────
// Boundary: [-128, 128] → i16 (exceeds i8 max by 1)
// ────────────────────────────────────────────────────

#[test]
fn boundary_just_exceeds_i8_narrows_to_i16() {
    let pool = Pool::new();
    let mut plan = ReprPlan::new(NarrowingPolicy::Aggressive);

    let idx = Idx::from_raw(Idx::FIRST_DYNAMIC);
    setup_struct(&mut plan, idx, vec![int_field(0)]);
    plan.join_field_range(idx, 0, ValueRange::Bounded { lo: -128, hi: 128 });

    narrow_struct_fields(&mut plan, &pool);

    assert_eq!(
        struct_field_width(&plan, idx, 0),
        Some(IntWidth::I16),
        "[-128, 128] exceeds signed i8 max (127) → i16"
    );
}

// ────────────────────────────────────────────────────
// Boundary: [-32768, 32767] → i16
// ────────────────────────────────────────────────────

#[test]
fn boundary_exact_i16_range_narrows_to_i16() {
    let pool = Pool::new();
    let mut plan = ReprPlan::new(NarrowingPolicy::Aggressive);

    let idx = Idx::from_raw(Idx::FIRST_DYNAMIC);
    setup_struct(&mut plan, idx, vec![int_field(0)]);
    plan.join_field_range(
        idx,
        0,
        ValueRange::Bounded {
            lo: -32_768,
            hi: 32_767,
        },
    );

    narrow_struct_fields(&mut plan, &pool);

    assert_eq!(
        struct_field_width(&plan, idx, 0),
        Some(IntWidth::I16),
        "[-32768, 32767] fits exactly in i16"
    );
}

// ────────────────────────────────────────────────────
// Boundary: [-32769, 0] → i32
// ────────────────────────────────────────────────────

#[test]
fn boundary_just_exceeds_i16_narrows_to_i32() {
    let pool = Pool::new();
    let mut plan = ReprPlan::new(NarrowingPolicy::Aggressive);

    let idx = Idx::from_raw(Idx::FIRST_DYNAMIC);
    setup_struct(&mut plan, idx, vec![int_field(0)]);
    plan.join_field_range(idx, 0, ValueRange::Bounded { lo: -32_769, hi: 0 });

    narrow_struct_fields(&mut plan, &pool);

    assert_eq!(
        struct_field_width(&plan, idx, 0),
        Some(IntWidth::I32),
        "[-32769, 0] exceeds i16 range → i32"
    );
}

// ────────────────────────────────────────────────────
// Top range → no narrowing (stays i64)
// ────────────────────────────────────────────────────

#[test]
fn top_range_stays_i64() {
    let pool = Pool::new();
    let mut plan = ReprPlan::new(NarrowingPolicy::Aggressive);

    let idx = Idx::from_raw(Idx::FIRST_DYNAMIC);
    setup_struct(&mut plan, idx, vec![int_field(0)]);
    // No field range set → defaults to Top.

    narrow_struct_fields(&mut plan, &pool);

    assert_eq!(
        struct_field_width(&plan, idx, 0),
        Some(IntWidth::I64),
        "Top range → no narrowing, stays i64"
    );
}

// ────────────────────────────────────────────────────
// Bottom range → i8 (unreachable code, smallest valid)
// ────────────────────────────────────────────────────

#[test]
fn bottom_range_narrows_to_i8() {
    let pool = Pool::new();
    let mut plan = ReprPlan::new(NarrowingPolicy::Aggressive);

    let idx = Idx::from_raw(Idx::FIRST_DYNAMIC);
    setup_struct(&mut plan, idx, vec![int_field(0)]);
    plan.join_field_range(idx, 0, ValueRange::Bottom);

    narrow_struct_fields(&mut plan, &pool);

    assert_eq!(
        struct_field_width(&plan, idx, 0),
        Some(IntWidth::I8),
        "Bottom range → i8 (unreachable, smallest valid)"
    );
}

// ────────────────────────────────────────────────────
// #repr("c") struct: NOT narrowed (ABI contract)
// ────────────────────────────────────────────────────

#[test]
fn repr_c_struct_not_narrowed() {
    let pool = Pool::new();
    let mut plan = ReprPlan::new(NarrowingPolicy::Aggressive);

    let idx = Idx::from_raw(Idx::FIRST_DYNAMIC);
    setup_struct(&mut plan, idx, vec![int_field(0)]);
    plan.set_repr_attr(idx, ReprAttribute::C);
    plan.join_field_range(idx, 0, ValueRange::Bounded { lo: 0, hi: 10 });

    narrow_struct_fields(&mut plan, &pool);

    assert_eq!(
        struct_field_width(&plan, idx, 0),
        Some(IntWidth::I64),
        "#repr(\"c\") struct must not be narrowed"
    );
}

// ────────────────────────────────────────────────────
// #repr("packed") struct: NOT narrowed
// ────────────────────────────────────────────────────

#[test]
fn repr_packed_struct_not_narrowed() {
    let pool = Pool::new();
    let mut plan = ReprPlan::new(NarrowingPolicy::Aggressive);

    let idx = Idx::from_raw(Idx::FIRST_DYNAMIC);
    setup_struct(&mut plan, idx, vec![int_field(0)]);
    plan.set_repr_attr(idx, ReprAttribute::Packed);
    plan.join_field_range(idx, 0, ValueRange::Bounded { lo: 0, hi: 10 });

    narrow_struct_fields(&mut plan, &pool);

    assert_eq!(
        struct_field_width(&plan, idx, 0),
        Some(IntWidth::I64),
        "#repr(\"packed\") struct must not be narrowed"
    );
}

// ────────────────────────────────────────────────────
// #repr("transparent") struct: NOT narrowed
// ────────────────────────────────────────────────────

#[test]
fn repr_transparent_struct_not_narrowed() {
    let pool = Pool::new();
    let mut plan = ReprPlan::new(NarrowingPolicy::Aggressive);

    let idx = Idx::from_raw(Idx::FIRST_DYNAMIC);
    setup_struct(&mut plan, idx, vec![int_field(0)]);
    plan.set_repr_attr(idx, ReprAttribute::Transparent);
    plan.join_field_range(idx, 0, ValueRange::Bounded { lo: 0, hi: 10 });

    narrow_struct_fields(&mut plan, &pool);

    assert_eq!(
        struct_field_width(&plan, idx, 0),
        Some(IntWidth::I64),
        "#repr(\"transparent\") struct must not be narrowed"
    );
}

// ────────────────────────────────────────────────────
// #repr("aligned", 8) struct: CAN be narrowed (alignment is independent)
// ────────────────────────────────────────────────────

#[test]
fn repr_aligned_struct_can_be_narrowed() {
    let pool = Pool::new();
    let mut plan = ReprPlan::new(NarrowingPolicy::Aggressive);

    let idx = Idx::from_raw(Idx::FIRST_DYNAMIC);
    setup_struct(&mut plan, idx, vec![int_field(0)]);
    plan.set_repr_attr(idx, ReprAttribute::Aligned(8));
    plan.join_field_range(idx, 0, ValueRange::Bounded { lo: 0, hi: 10 });

    narrow_struct_fields(&mut plan, &pool);

    assert_eq!(
        struct_field_width(&plan, idx, 0),
        Some(IntWidth::I8),
        "#repr(\"aligned\", 8) does not prevent field narrowing"
    );
}

// ────────────────────────────────────────────────────
// #repr("c", aligned N) struct: NOT narrowed (C-ABI contract)
// ────────────────────────────────────────────────────

#[test]
fn repr_c_aligned_struct_not_narrowed() {
    let pool = Pool::new();
    let mut plan = ReprPlan::new(NarrowingPolicy::Aggressive);

    let idx = Idx::from_raw(Idx::FIRST_DYNAMIC);
    setup_struct(&mut plan, idx, vec![int_field(0)]);
    plan.set_repr_attr(idx, ReprAttribute::CAligned(16));
    plan.join_field_range(idx, 0, ValueRange::Bounded { lo: 0, hi: 10 });

    narrow_struct_fields(&mut plan, &pool);

    assert_eq!(
        struct_field_width(&plan, idx, 0),
        Some(IntWidth::I64),
        "#repr(\"c\", aligned 16) struct must not be narrowed — C-ABI layout is fixed"
    );
}

// ────────────────────────────────────────────────────
// Public type: NOT narrowed (ABI contract)
// ────────────────────────────────────────────────────

#[test]
fn public_type_not_narrowed() {
    let pool = Pool::new();
    let mut plan = ReprPlan::new(NarrowingPolicy::Aggressive);

    let idx = Idx::from_raw(Idx::FIRST_DYNAMIC);
    setup_struct(&mut plan, idx, vec![int_field(0)]);
    plan.set_pub_type_indices(std::iter::once(idx));
    plan.join_field_range(idx, 0, ValueRange::Bounded { lo: 0, hi: 10 });

    narrow_struct_fields(&mut plan, &pool);

    assert_eq!(
        struct_field_width(&plan, idx, 0),
        Some(IntWidth::I64),
        "public type must not be narrowed — ABI contract with external code"
    );
}

// ────────────────────────────────────────────────────
// Private type: IS narrowed normally complement
// ────────────────────────────────────────────────────

#[test]
fn private_type_narrowed_normally() {
    let pool = Pool::new();
    let mut plan = ReprPlan::new(NarrowingPolicy::Aggressive);

    let pub_idx = Idx::from_raw(Idx::FIRST_DYNAMIC);
    let priv_idx = Idx::from_raw(Idx::FIRST_DYNAMIC + 1);
    setup_struct(&mut plan, pub_idx, vec![int_field(0)]);
    setup_struct(&mut plan, priv_idx, vec![int_field(0)]);
    // Only pub_idx is public.
    plan.set_pub_type_indices(std::iter::once(pub_idx));
    plan.join_field_range(pub_idx, 0, ValueRange::Bounded { lo: 0, hi: 10 });
    plan.join_field_range(priv_idx, 0, ValueRange::Bounded { lo: 0, hi: 10 });

    narrow_struct_fields(&mut plan, &pool);

    // Public type stays canonical.
    assert_eq!(
        struct_field_width(&plan, pub_idx, 0),
        Some(IntWidth::I64),
        "public type must not be narrowed"
    );
    // Private type is narrowed.
    assert_eq!(
        struct_field_width(&plan, priv_idx, 0),
        Some(IntWidth::I8),
        "private type should be narrowed normally"
    );
}

// ────────────────────────────────────────────────────
// NarrowingPolicy::Disabled → no narrowing
// ────────────────────────────────────────────────────

#[test]
fn disabled_policy_skips_narrowing() {
    let pool = Pool::new();
    let mut plan = ReprPlan::new(NarrowingPolicy::Disabled);

    let idx = Idx::from_raw(Idx::FIRST_DYNAMIC);
    setup_struct(&mut plan, idx, vec![int_field(0)]);
    plan.join_field_range(idx, 0, ValueRange::Bounded { lo: 0, hi: 10 });

    narrow_struct_fields(&mut plan, &pool);

    assert_eq!(
        struct_field_width(&plan, idx, 0),
        Some(IntWidth::I64),
        "Disabled policy must not narrow anything"
    );
}

// ────────────────────────────────────────────────────
// Non-int fields are left alone
// ────────────────────────────────────────────────────

#[test]
fn non_int_fields_untouched() {
    let pool = Pool::new();
    let mut plan = ReprPlan::new(NarrowingPolicy::Aggressive);

    let idx = Idx::from_raw(Idx::FIRST_DYNAMIC);
    setup_struct(&mut plan, idx, vec![bool_field(0), int_field(1)]);
    plan.join_field_range(idx, 1, ValueRange::Bounded { lo: 0, hi: 10 });

    narrow_struct_fields(&mut plan, &pool);

    // Bool field unchanged.
    match plan.get_repr(idx) {
        Some(MachineRepr::Struct(s)) => {
            assert_eq!(s.fields[0].repr, MachineRepr::Bool, "bool field untouched");
            assert_eq!(
                s.fields[1].repr,
                MachineRepr::Int {
                    width: IntWidth::I8,
                    signed: true,
                },
                "int field narrowed to i8"
            );
        }
        other => panic!("expected Struct, got {other:?}"),
    }
}

// ────────────────────────────────────────────────────
// Already-narrowed field (not I64) is left alone
// ────────────────────────────────────────────────────

#[test]
fn already_narrow_field_untouched() {
    let pool = Pool::new();
    let mut plan = ReprPlan::new(NarrowingPolicy::Aggressive);

    let idx = Idx::from_raw(Idx::FIRST_DYNAMIC);
    setup_struct(&mut plan, idx, vec![int_field_width(0, IntWidth::I32)]);
    plan.join_field_range(idx, 0, ValueRange::Bounded { lo: 0, hi: 10 });

    narrow_struct_fields(&mut plan, &pool);

    assert_eq!(
        struct_field_width(&plan, idx, 0),
        Some(IntWidth::I32),
        "already-narrow I32 field should not be changed"
    );
}

// ────────────────────────────────────────────────────
// Tuple element narrowing
// ────────────────────────────────────────────────────

#[test]
fn tuple_elements_not_narrowed_phase_a() {
    // Phase A: tuples are skipped — they're used as collection elements,
    // iterator state, and intermediate values where element_store_size()
    // assumes canonical field widths. Tuple narrowing is Phase C.
    let pool = Pool::new();
    let mut plan = ReprPlan::new(NarrowingPolicy::Aggressive);

    let idx = Idx::from_raw(Idx::FIRST_DYNAMIC);
    setup_tuple(&mut plan, idx, vec![int_field(0), int_field(1)]);
    plan.join_field_range(idx, 0, ValueRange::Bounded { lo: -128, hi: 127 });
    plan.join_field_range(
        idx,
        1,
        ValueRange::Bounded {
            lo: -32_768,
            hi: 32_767,
        },
    );

    narrow_struct_fields(&mut plan, &pool);

    // Tuple fields stay at I64 (canonical) — not narrowed in Phase A.
    assert_eq!(
        tuple_element_width(&plan, idx, 0),
        Some(IntWidth::I64),
        "tuple element 0 should stay i64 (Phase A skips tuples)"
    );
    assert_eq!(
        tuple_element_width(&plan, idx, 1),
        Some(IntWidth::I64),
        "tuple element 1 should stay i64 (Phase A skips tuples)"
    );
}

// ────────────────────────────────────────────────────
// Mixed: some fields narrow, some stay i64
// ────────────────────────────────────────────────────

#[test]
fn mixed_fields_partial_narrowing() {
    let pool = Pool::new();
    let mut plan = ReprPlan::new(NarrowingPolicy::Aggressive);

    let idx = Idx::from_raw(Idx::FIRST_DYNAMIC);
    setup_struct(
        &mut plan,
        idx,
        vec![int_field(0), int_field(1), int_field(2)],
    );
    plan.join_field_range(idx, 0, ValueRange::Bounded { lo: 0, hi: 100 });
    // Field 1: no range set → Top → stays I64.
    plan.join_field_range(
        idx,
        2,
        ValueRange::Bounded {
            lo: -2_000_000_000,
            hi: 2_000_000_000,
        },
    );

    narrow_struct_fields(&mut plan, &pool);

    assert_eq!(struct_field_width(&plan, idx, 0), Some(IntWidth::I8));
    assert_eq!(struct_field_width(&plan, idx, 1), Some(IntWidth::I64));
    assert_eq!(struct_field_width(&plan, idx, 2), Some(IntWidth::I32));
}

// ────────────────────────────────────────────────────
// Empty struct: no panic, no narrowing
// ────────────────────────────────────────────────────

#[test]
fn empty_struct_no_panic() {
    let pool = Pool::new();
    let mut plan = ReprPlan::new(NarrowingPolicy::Aggressive);

    let idx = Idx::from_raw(Idx::FIRST_DYNAMIC);
    setup_struct(&mut plan, idx, vec![]);

    narrow_struct_fields(&mut plan, &pool);

    // Should not panic or change anything.
    match plan.get_repr(idx) {
        Some(MachineRepr::Struct(s)) => assert!(s.fields.is_empty()),
        other => panic!("expected empty Struct, got {other:?}"),
    }
}

// ────────────────────────────────────────────────────
// i32 range boundary: [-2^31, 2^31-1] → i32
// ────────────────────────────────────────────────────

#[test]
fn boundary_exact_i32_range() {
    let pool = Pool::new();
    let mut plan = ReprPlan::new(NarrowingPolicy::Aggressive);

    let idx = Idx::from_raw(Idx::FIRST_DYNAMIC);
    setup_struct(&mut plan, idx, vec![int_field(0)]);
    plan.join_field_range(
        idx,
        0,
        ValueRange::Bounded {
            lo: -2_147_483_648,
            hi: 2_147_483_647,
        },
    );

    narrow_struct_fields(&mut plan, &pool);

    assert_eq!(
        struct_field_width(&plan, idx, 0),
        Some(IntWidth::I32),
        "[-2^31, 2^31-1] fits exactly in i32"
    );
}

// ────────────────────────────────────────────────────
// Exceeds i32: [-2^31, 2^31] → i64
// ────────────────────────────────────────────────────

#[test]
fn boundary_just_exceeds_i32_stays_i64() {
    let pool = Pool::new();
    let mut plan = ReprPlan::new(NarrowingPolicy::Aggressive);

    let idx = Idx::from_raw(Idx::FIRST_DYNAMIC);
    setup_struct(&mut plan, idx, vec![int_field(0)]);
    plan.join_field_range(
        idx,
        0,
        ValueRange::Bounded {
            lo: -2_147_483_648,
            hi: 2_147_483_648,
        },
    );

    narrow_struct_fields(&mut plan, &pool);

    assert_eq!(
        struct_field_width(&plan, idx, 0),
        Some(IntWidth::I64),
        "[-2^31, 2^31] exceeds i32 range → stays i64"
    );
}

// ────────────────────────────────────────────────────
// Single constant value: [42, 42] → i8
// ────────────────────────────────────────────────────

#[test]
fn constant_value_narrows_to_i8() {
    let pool = Pool::new();
    let mut plan = ReprPlan::new(NarrowingPolicy::Aggressive);

    let idx = Idx::from_raw(Idx::FIRST_DYNAMIC);
    setup_struct(&mut plan, idx, vec![int_field(0)]);
    plan.join_field_range(idx, 0, ValueRange::Bounded { lo: 42, hi: 42 });

    narrow_struct_fields(&mut plan, &pool);

    assert_eq!(
        struct_field_width(&plan, idx, 0),
        Some(IntWidth::I8),
        "[42, 42] fits in i8"
    );
}

// ────────────────────────────────────────────────────
// FieldRepr.offset stays zero (§04/§06 interface contract)
// ────────────────────────────────────────────────────

#[test]
fn field_offset_stays_zero_after_narrowing() {
    let pool = Pool::new();
    let mut plan = ReprPlan::new(NarrowingPolicy::Aggressive);

    let idx = Idx::from_raw(Idx::FIRST_DYNAMIC);
    setup_struct(&mut plan, idx, vec![int_field(0), int_field(1)]);
    plan.join_field_range(idx, 0, ValueRange::Bounded { lo: 0, hi: 10 });
    plan.join_field_range(idx, 1, ValueRange::Bounded { lo: 0, hi: 10 });

    narrow_struct_fields(&mut plan, &pool);

    match plan.get_repr(idx) {
        Some(MachineRepr::Struct(s)) => {
            for field in &s.fields {
                assert_eq!(
                    field.offset, 0,
                    "§04 must not set offsets — §06 is the authority"
                );
            }
        }
        other => panic!("expected Struct, got {other:?}"),
    }
}

// ════════════════════════════════════════════════════
// §04.2 — ABI Boundary Classification Tests
// ════════════════════════════════════════════════════

// Boundary classification priority

#[test]
fn classify_ffi_boundary() {
    let info = FunctionBoundaryInfo {
        is_ffi: true,
        is_public: true,
        is_trait_method: true,
        is_closure: true,
    };
    assert_eq!(
        abi::classify_function_boundary(info),
        AbiBoundary::Ffi,
        "FFI is highest priority"
    );
}

#[test]
fn classify_public_api_boundary() {
    let info = FunctionBoundaryInfo {
        is_public: true,
        is_trait_method: true,
        is_closure: true,
        ..Default::default()
    };
    assert_eq!(
        abi::classify_function_boundary(info),
        AbiBoundary::PublicApi,
        "public API outranks trait method"
    );
}

#[test]
fn classify_trait_method_boundary() {
    let info = FunctionBoundaryInfo {
        is_trait_method: true,
        is_closure: true,
        ..Default::default()
    };
    assert_eq!(
        abi::classify_function_boundary(info),
        AbiBoundary::TraitMethod,
        "trait method outranks closure"
    );
}

#[test]
fn classify_closure_boundary() {
    let info = FunctionBoundaryInfo {
        is_closure: true,
        ..Default::default()
    };
    assert_eq!(
        abi::classify_function_boundary(info),
        AbiBoundary::ClosureCapture
    );
}

#[test]
fn classify_internal_call_boundary() {
    let info = FunctionBoundaryInfo::default();
    assert_eq!(
        abi::classify_function_boundary(info),
        AbiBoundary::InternalCall,
        "all flags false → internal"
    );
}

// Width requirements per boundary

#[test]
fn ffi_requires_platform_cabi() {
    assert_eq!(
        abi::width_requirement(AbiBoundary::Ffi),
        WidthRequirement::PlatformCabi,
    );
}

#[test]
fn public_api_requires_canonical() {
    assert_eq!(
        abi::width_requirement(AbiBoundary::PublicApi),
        WidthRequirement::Canonical,
    );
}

#[test]
fn trait_method_requires_canonical() {
    assert_eq!(
        abi::width_requirement(AbiBoundary::TraitMethod),
        WidthRequirement::Canonical,
    );
}

#[test]
fn closure_capture_requires_canonical() {
    assert_eq!(
        abi::width_requirement(AbiBoundary::ClosureCapture),
        WidthRequirement::Canonical,
    );
}

#[test]
fn internal_call_allows_narrow_if_agreed() {
    assert_eq!(
        abi::width_requirement(AbiBoundary::InternalCall),
        WidthRequirement::NarrowIfAgreed,
    );
}

// can_narrow_param / can_narrow_return

#[test]
fn only_internal_call_allows_narrow_param() {
    assert!(abi::can_narrow_param(AbiBoundary::InternalCall));
    assert!(!abi::can_narrow_param(AbiBoundary::PublicApi));
    assert!(!abi::can_narrow_param(AbiBoundary::Ffi));
    assert!(!abi::can_narrow_param(AbiBoundary::TraitMethod));
    assert!(!abi::can_narrow_param(AbiBoundary::ClosureCapture));
}

#[test]
fn only_internal_call_allows_narrow_return() {
    assert!(abi::can_narrow_return(AbiBoundary::InternalCall));
    assert!(!abi::can_narrow_return(AbiBoundary::PublicApi));
    assert!(!abi::can_narrow_return(AbiBoundary::Ffi));
    assert!(!abi::can_narrow_return(AbiBoundary::TraitMethod));
    assert!(!abi::can_narrow_return(AbiBoundary::ClosureCapture));
}

// Cross-module agreement

#[test]
fn cross_module_agreed_allows_narrowing() {
    assert!(abi::can_narrow_cross_module(CrossModuleAgreement::Agreed));
}

#[test]
fn cross_module_disagreed_blocks_narrowing() {
    assert!(!abi::can_narrow_cross_module(
        CrossModuleAgreement::Disagreed
    ));
}

#[test]
fn cross_module_unknown_blocks_narrowing() {
    assert!(!abi::can_narrow_cross_module(CrossModuleAgreement::Unknown));
}

// Effective boundary width

#[test]
fn effective_width_public_api_always_i64() {
    assert_eq!(
        abi::effective_boundary_width(
            IntWidth::I8,
            AbiBoundary::PublicApi,
            CrossModuleAgreement::Agreed
        ),
        IntWidth::I64,
        "public API always canonical regardless of agreement"
    );
}

#[test]
fn effective_width_ffi_always_i64() {
    assert_eq!(
        abi::effective_boundary_width(
            IntWidth::I16,
            AbiBoundary::Ffi,
            CrossModuleAgreement::Agreed
        ),
        IntWidth::I64,
        "FFI boundary returns canonical (Ori int → i64 in C)"
    );
}

#[test]
fn effective_width_internal_agreed_preserves_narrow() {
    assert_eq!(
        abi::effective_boundary_width(
            IntWidth::I8,
            AbiBoundary::InternalCall,
            CrossModuleAgreement::Agreed
        ),
        IntWidth::I8,
        "internal call with agreement keeps narrow width"
    );
}

#[test]
fn effective_width_internal_disagreed_widens_to_i64() {
    assert_eq!(
        abi::effective_boundary_width(
            IntWidth::I8,
            AbiBoundary::InternalCall,
            CrossModuleAgreement::Disagreed
        ),
        IntWidth::I64,
        "internal call without agreement widens to canonical"
    );
}

#[test]
fn effective_width_internal_unknown_widens_to_i64() {
    assert_eq!(
        abi::effective_boundary_width(
            IntWidth::I16,
            AbiBoundary::InternalCall,
            CrossModuleAgreement::Unknown
        ),
        IntWidth::I64,
        "unknown agreement is conservative → canonical"
    );
}

// sext / trunc boundary detection

#[test]
fn needs_sext_at_public_boundary() {
    assert!(
        abi::needs_sext_at_boundary(
            IntWidth::I8,
            AbiBoundary::PublicApi,
            CrossModuleAgreement::Unknown
        ),
        "narrowed i8 value needs sext to i64 at public boundary"
    );
}

#[test]
fn no_sext_for_canonical_at_public_boundary() {
    assert!(
        !abi::needs_sext_at_boundary(
            IntWidth::I64,
            AbiBoundary::PublicApi,
            CrossModuleAgreement::Unknown
        ),
        "i64 value at public boundary needs no widening"
    );
}

#[test]
fn no_sext_at_internal_agreed_boundary() {
    assert!(
        !abi::needs_sext_at_boundary(
            IntWidth::I16,
            AbiBoundary::InternalCall,
            CrossModuleAgreement::Agreed
        ),
        "internal call with agreement — no widening needed"
    );
}

#[test]
fn needs_sext_at_internal_disagreed_boundary() {
    assert!(
        abi::needs_sext_at_boundary(
            IntWidth::I16,
            AbiBoundary::InternalCall,
            CrossModuleAgreement::Disagreed
        ),
        "internal call without agreement — must widen"
    );
}

#[test]
fn needs_trunc_after_public_boundary() {
    assert!(
        abi::needs_trunc_after_boundary(
            IntWidth::I8,
            AbiBoundary::PublicApi,
            CrossModuleAgreement::Unknown
        ),
        "canonical value arriving at narrowed local needs trunc"
    );
}

#[test]
fn no_trunc_when_already_canonical() {
    assert!(
        !abi::needs_trunc_after_boundary(
            IntWidth::I64,
            AbiBoundary::PublicApi,
            CrossModuleAgreement::Unknown
        ),
        "i64 local at public boundary needs no trunc"
    );
}

// Semantic pin: all boundary widths across all AbiBoundary variants

#[test]
fn semantic_pin_all_boundaries_width_matrix() {
    let narrow = IntWidth::I8;
    let agreed = CrossModuleAgreement::Agreed;
    let unknown = CrossModuleAgreement::Unknown;

    // FFI → always i64
    assert_eq!(
        abi::effective_boundary_width(narrow, AbiBoundary::Ffi, agreed),
        IntWidth::I64
    );
    // Public → always i64
    assert_eq!(
        abi::effective_boundary_width(narrow, AbiBoundary::PublicApi, agreed),
        IntWidth::I64
    );
    // Trait → always i64
    assert_eq!(
        abi::effective_boundary_width(narrow, AbiBoundary::TraitMethod, agreed),
        IntWidth::I64
    );
    // Closure → always i64
    assert_eq!(
        abi::effective_boundary_width(narrow, AbiBoundary::ClosureCapture, agreed),
        IntWidth::I64
    );
    // Internal + agreed → narrow
    assert_eq!(
        abi::effective_boundary_width(narrow, AbiBoundary::InternalCall, agreed),
        IntWidth::I8
    );
    // Internal + unknown → i64
    assert_eq!(
        abi::effective_boundary_width(narrow, AbiBoundary::InternalCall, unknown),
        IntWidth::I64
    );
}

// Semantic pin: narrowing width matrix (i8/i16/i32/i64 × boundary)

#[test]
fn semantic_pin_width_preservation_internal_agreed() {
    let agreed = CrossModuleAgreement::Agreed;
    let internal = AbiBoundary::InternalCall;

    assert_eq!(
        abi::effective_boundary_width(IntWidth::I8, internal, agreed),
        IntWidth::I8
    );
    assert_eq!(
        abi::effective_boundary_width(IntWidth::I16, internal, agreed),
        IntWidth::I16
    );
    assert_eq!(
        abi::effective_boundary_width(IntWidth::I32, internal, agreed),
        IntWidth::I32
    );
    assert_eq!(
        abi::effective_boundary_width(IntWidth::I64, internal, agreed),
        IntWidth::I64
    );
}

// ════════════════════════════════════════════════════
// §04.3 — Overflow Guard Insertion Tests
// ════════════════════════════════════════════════════

// can_overflow: addition

#[test]
fn add_no_overflow_in_i8() {
    // [0, 50] + [0, 50] = [0, 100] — fits in i8 [-128, 127]
    let lhs = ValueRange::Bounded { lo: 0, hi: 50 };
    let rhs = ValueRange::Bounded { lo: 0, hi: 50 };
    assert!(
        !overflow::can_overflow(BinaryOp::Add, lhs, rhs, IntWidth::I8),
        "[0,50] + [0,50] = [0,100] fits in i8"
    );
}

#[test]
fn add_overflows_i8() {
    // [0, 100] + [0, 100] = [0, 200] — does NOT fit in i8 [-128, 127]
    let lhs = ValueRange::Bounded { lo: 0, hi: 100 };
    let rhs = ValueRange::Bounded { lo: 0, hi: 100 };
    assert!(
        overflow::can_overflow(BinaryOp::Add, lhs, rhs, IntWidth::I8),
        "[0,100] + [0,100] = [0,200] overflows i8"
    );
}

#[test]
fn add_fits_in_i16_not_i8() {
    // [0, 100] + [0, 100] = [0, 200] — overflows i8 but fits i16
    let lhs = ValueRange::Bounded { lo: 0, hi: 100 };
    let rhs = ValueRange::Bounded { lo: 0, hi: 100 };
    assert!(overflow::can_overflow(
        BinaryOp::Add,
        lhs,
        rhs,
        IntWidth::I8
    ));
    assert!(!overflow::can_overflow(
        BinaryOp::Add,
        lhs,
        rhs,
        IntWidth::I16
    ));
}

// can_overflow: subtraction

#[test]
fn sub_no_overflow_in_i8() {
    // [0, 50] - [0, 50] = [-50, 50] — fits in i8
    let lhs = ValueRange::Bounded { lo: 0, hi: 50 };
    let rhs = ValueRange::Bounded { lo: 0, hi: 50 };
    assert!(!overflow::can_overflow(
        BinaryOp::Sub,
        lhs,
        rhs,
        IntWidth::I8
    ));
}

#[test]
fn sub_overflows_i8() {
    // [0, 127] - [-127, 0] = [0, 254] — overflows i8
    let lhs = ValueRange::Bounded { lo: 0, hi: 127 };
    let rhs = ValueRange::Bounded { lo: -127, hi: 0 };
    assert!(overflow::can_overflow(
        BinaryOp::Sub,
        lhs,
        rhs,
        IntWidth::I8
    ));
}

// can_overflow: multiplication

#[test]
fn mul_no_overflow_in_i8() {
    // [0, 10] * [0, 10] = [0, 100] — fits in i8
    let lhs = ValueRange::Bounded { lo: 0, hi: 10 };
    let rhs = ValueRange::Bounded { lo: 0, hi: 10 };
    assert!(!overflow::can_overflow(
        BinaryOp::Mul,
        lhs,
        rhs,
        IntWidth::I8
    ));
}

#[test]
fn mul_overflows_i8() {
    // [0, 20] * [0, 20] = [0, 400] — overflows i8
    let lhs = ValueRange::Bounded { lo: 0, hi: 20 };
    let rhs = ValueRange::Bounded { lo: 0, hi: 20 };
    assert!(overflow::can_overflow(
        BinaryOp::Mul,
        lhs,
        rhs,
        IntWidth::I8
    ));
}

// can_overflow: non-arithmetic ops conservatively report overflow

#[test]
fn comparison_op_conservative_overflow() {
    let lhs = ValueRange::Bounded { lo: 0, hi: 10 };
    let rhs = ValueRange::Bounded { lo: 0, hi: 10 };
    // Comparison ops return Top → can_overflow returns true (conservative)
    assert!(overflow::can_overflow(BinaryOp::Eq, lhs, rhs, IntWidth::I8));
}

// can_overflow: Top range always overflows non-I64 target

#[test]
fn top_range_always_overflows_narrow() {
    assert!(overflow::can_overflow(
        BinaryOp::Add,
        ValueRange::Top,
        ValueRange::Top,
        IntWidth::I8
    ));
    assert!(overflow::can_overflow(
        BinaryOp::Add,
        ValueRange::Top,
        ValueRange::Top,
        IntWidth::I16
    ));
    assert!(overflow::can_overflow(
        BinaryOp::Add,
        ValueRange::Top,
        ValueRange::Top,
        IntWidth::I32
    ));
}

// can_overflow: Bottom range never overflows

#[test]
fn bottom_range_never_overflows() {
    // Bottom + anything = Bottom → fits in any width
    assert!(!overflow::can_overflow(
        BinaryOp::Add,
        ValueRange::Bottom,
        ValueRange::Bottom,
        IntWidth::I8
    ));
}

// recommend_strategy

#[test]
fn strategy_proven_safe_when_no_overflow() {
    let lhs = ValueRange::Bounded { lo: 0, hi: 50 };
    let rhs = ValueRange::Bounded { lo: 0, hi: 50 };
    assert_eq!(
        overflow::recommend_strategy(BinaryOp::Add, lhs, rhs, IntWidth::I8),
        OverflowStrategy::ProvenSafe,
    );
}

#[test]
fn strategy_widen_when_fits_in_next_wider() {
    // [0, 100] + [0, 100] = [0, 200] — overflows i8, fits i16
    let lhs = ValueRange::Bounded { lo: 0, hi: 100 };
    let rhs = ValueRange::Bounded { lo: 0, hi: 100 };
    assert_eq!(
        overflow::recommend_strategy(BinaryOp::Add, lhs, rhs, IntWidth::I8),
        OverflowStrategy::WidenCompute {
            intermediate_width: IntWidth::I16,
        },
    );
}

#[test]
fn strategy_widen_i16_to_i32() {
    // [0, 30000] + [0, 30000] = [0, 60000] — overflows i16, fits i32
    let lhs = ValueRange::Bounded { lo: 0, hi: 30_000 };
    let rhs = ValueRange::Bounded { lo: 0, hi: 30_000 };
    assert_eq!(
        overflow::recommend_strategy(BinaryOp::Add, lhs, rhs, IntWidth::I16),
        OverflowStrategy::WidenCompute {
            intermediate_width: IntWidth::I32,
        },
    );
}

#[test]
fn strategy_widen_to_i64_for_large_multiplication() {
    // [0, 100_000] * [0, 100_000] = [0, 10_000_000_000] — overflows i32
    // Next wider is I64, and the result fits in I64 → WidenCompute { I64 }
    //
    // Note: UseCanonical is currently unreachable with signed i64 as the
    // canonical type — any overflow at I8/I16/I32 always fits in the
    // next-wider type, and I64 is the ceiling. UseCanonical exists for
    // forward compatibility (future unsigned narrowing or i128).
    let lhs = ValueRange::Bounded { lo: 0, hi: 100_000 };
    let rhs = ValueRange::Bounded { lo: 0, hi: 100_000 };
    assert_eq!(
        overflow::recommend_strategy(BinaryOp::Mul, lhs, rhs, IntWidth::I32),
        OverflowStrategy::WidenCompute {
            intermediate_width: IntWidth::I64,
        },
    );
}

#[test]
fn strategy_canonical_for_i64_target() {
    // I64 has no next-wider type → UseCanonical when overflow detected
    // But Top + Top at I64 doesn't overflow I64 since fits_in(I64) returns true for Top
    // So let's use a case where the transfer function returns Top
    let lhs = ValueRange::Top;
    let rhs = ValueRange::Top;
    // For Add with Top inputs, range_add returns Top, and Top.fits_in(I64) = true
    // So this is actually ProvenSafe for i64 target
    assert_eq!(
        overflow::recommend_strategy(BinaryOp::Add, lhs, rhs, IntWidth::I64),
        OverflowStrategy::ProvenSafe,
        "i64 target with Top range is proven safe (canonical = no overflow)"
    );
}

// Semantic pin: strategy progression (ProvenSafe < WidenCompute < UseCanonical)

#[test]
fn semantic_pin_strategy_progression() {
    // Same operation, decreasing target width → escalating strategies
    let lhs = ValueRange::Bounded { lo: 0, hi: 100 };
    let rhs = ValueRange::Bounded { lo: 0, hi: 100 };

    // At i16: [0,200] fits → ProvenSafe
    assert_eq!(
        overflow::recommend_strategy(BinaryOp::Add, lhs, rhs, IntWidth::I16),
        OverflowStrategy::ProvenSafe,
    );

    // At i8: [0,200] overflows i8, fits i16 → WidenCompute to i16
    assert_eq!(
        overflow::recommend_strategy(BinaryOp::Add, lhs, rhs, IntWidth::I8),
        OverflowStrategy::WidenCompute {
            intermediate_width: IntWidth::I16,
        },
    );
}

// Semantic pin: all arithmetic ops × overflow detection

#[test]
fn semantic_pin_all_arithmetic_ops_overflow_detection() {
    let small = ValueRange::Bounded { lo: 0, hi: 10 };
    let large = ValueRange::Bounded { lo: 0, hi: 200 };

    // Add: [0,10] + [0,200] = [0,210] — overflows i8
    assert!(overflow::can_overflow(
        BinaryOp::Add,
        small,
        large,
        IntWidth::I8
    ));

    // Sub: [0,10] - [0,200] = [-200,10] — overflows i8
    assert!(overflow::can_overflow(
        BinaryOp::Sub,
        small,
        large,
        IntWidth::I8
    ));

    // Mul: [0,10] * [0,200] = [0,2000] — overflows i8
    assert!(overflow::can_overflow(
        BinaryOp::Mul,
        small,
        large,
        IntWidth::I8
    ));

    // Div: [0,200] / [1,10] — result ≤ 200, overflows i8
    let divisor = ValueRange::Bounded { lo: 1, hi: 10 };
    assert!(overflow::can_overflow(
        BinaryOp::Div,
        large,
        divisor,
        IntWidth::I8
    ));

    // Mod: [0,200] % [1,10] — result ∈ [0,9], fits i8
    assert!(!overflow::can_overflow(
        BinaryOp::Mod,
        large,
        divisor,
        IntWidth::I8
    ));

    // Shl: [0,10] << [0,3] — result up to 80, fits i8
    let shift = ValueRange::Bounded { lo: 0, hi: 3 };
    assert!(!overflow::can_overflow(
        BinaryOp::Shl,
        small,
        shift,
        IntWidth::I8
    ));

    // Shl: [0,10] << [0,8] — result up to 2560, overflows i8
    let big_shift = ValueRange::Bounded { lo: 0, hi: 8 };
    assert!(overflow::can_overflow(
        BinaryOp::Shl,
        small,
        big_shift,
        IntWidth::I8
    ));
}

// Phase C — Collection element narrowing

/// Helper: set up a list collection type `[int]` in the plan.
fn setup_list_collection(plan: &mut ReprPlan, collection_idx: Idx) {
    plan.set_repr(
        collection_idx,
        ReprDecision {
            source: DecisionSource::Canonical,
            type_idx: collection_idx,
            repr: MachineRepr::FatPointer(FatRepr::Collection {
                element_repr: Box::new(MachineRepr::Int {
                    width: IntWidth::I64,
                    signed: true,
                }),
            }),
            reason: DecisionReason::Canonical,
        },
    );
}

/// Helper: get the element int width of a collection after narrowing.
fn collection_element_width(plan: &ReprPlan, idx: Idx) -> Option<IntWidth> {
    match plan.get_repr(idx)? {
        MachineRepr::FatPointer(FatRepr::Collection { element_repr }) => {
            match element_repr.as_ref() {
                MachineRepr::Int { width, .. } => Some(*width),
                _ => None,
            }
        }
        _ => None,
    }
}

#[test]
fn phase_c_list_bounded_elements_narrow_to_i8() {
    let mut pool = Pool::default();
    let list_int = pool.list(Idx::INT);

    let mut plan = ReprPlan::new(NarrowingPolicy::Aggressive);
    setup_list_collection(&mut plan, list_int);

    // All elements in [-128, 127] → should narrow to i8.
    plan.join_element_range(list_int, ValueRange::Bounded { lo: -128, hi: 127 });

    narrow_collection_elements(&mut plan, &pool);

    assert_eq!(
        collection_element_width(&plan, list_int),
        Some(IntWidth::I8),
        "list [int] with elements [-128, 127] should narrow to i8"
    );
}

#[test]
fn phase_c_list_bounded_elements_narrow_to_i16() {
    let mut pool = Pool::default();
    let list_int = pool.list(Idx::INT);

    let mut plan = ReprPlan::new(NarrowingPolicy::Aggressive);
    setup_list_collection(&mut plan, list_int);

    // Elements in [0, 255] → signed i8 max is 127, so → i16.
    plan.join_element_range(list_int, ValueRange::Bounded { lo: 0, hi: 255 });

    narrow_collection_elements(&mut plan, &pool);

    assert_eq!(
        collection_element_width(&plan, list_int),
        Some(IntWidth::I16),
        "list [int] with elements [0, 255] should narrow to i16 (signed)"
    );
}

#[test]
fn phase_c_list_top_range_stays_i64() {
    let mut pool = Pool::default();
    let list_int = pool.list(Idx::INT);

    let mut plan = ReprPlan::new(NarrowingPolicy::Aggressive);
    setup_list_collection(&mut plan, list_int);

    // No element observations → Top → stays i64.
    narrow_collection_elements(&mut plan, &pool);

    assert_eq!(
        collection_element_width(&plan, list_int),
        Some(IntWidth::I64),
        "list [int] with Top range should stay i64"
    );
}

#[test]
fn phase_c_public_collection_not_narrowed() {
    let mut pool = Pool::default();
    let list_int = pool.list(Idx::INT);

    let mut plan = ReprPlan::new(NarrowingPolicy::Aggressive);
    setup_list_collection(&mut plan, list_int);
    plan.set_pub_type_indices([list_int]);

    // Even with bounded elements, public types are not narrowed.
    plan.join_element_range(list_int, ValueRange::Bounded { lo: 0, hi: 10 });

    narrow_collection_elements(&mut plan, &pool);

    assert_eq!(
        collection_element_width(&plan, list_int),
        Some(IntWidth::I64),
        "public [int] should not be narrowed"
    );
}

#[test]
fn phase_c_disabled_policy_suppresses_narrowing() {
    let mut pool = Pool::default();
    let list_int = pool.list(Idx::INT);

    let mut plan = ReprPlan::new(NarrowingPolicy::Disabled);
    setup_list_collection(&mut plan, list_int);
    plan.join_element_range(list_int, ValueRange::Bounded { lo: 0, hi: 10 });

    narrow_collection_elements(&mut plan, &pool);

    assert_eq!(
        collection_element_width(&plan, list_int),
        Some(IntWidth::I64),
        "NarrowingPolicy::Disabled should suppress all collection narrowing"
    );
}

#[test]
fn phase_c_repr_c_collection_not_narrowed() {
    let mut pool = Pool::default();
    let list_int = pool.list(Idx::INT);

    let mut plan = ReprPlan::new(NarrowingPolicy::Aggressive);
    setup_list_collection(&mut plan, list_int);
    plan.set_repr_attr(list_int, ReprAttribute::C);
    plan.join_element_range(list_int, ValueRange::Bounded { lo: 0, hi: 10 });

    narrow_collection_elements(&mut plan, &pool);

    assert_eq!(
        collection_element_width(&plan, list_int),
        Some(IntWidth::I64),
        "#repr(\"c\") collection should not be narrowed"
    );
}

#[test]
fn phase_c_semantic_pin_only_passes_with_narrowing() {
    // Semantic pin: this test ONLY passes if collection element narrowing is
    // active. Without it, the element width would remain I64.
    let mut pool = Pool::default();
    let list_int = pool.list(Idx::INT);

    let mut plan = ReprPlan::new(NarrowingPolicy::Aggressive);
    setup_list_collection(&mut plan, list_int);

    // Bounded elements [-50, 50] fit in i8.
    plan.join_element_range(list_int, ValueRange::Bounded { lo: -50, hi: 50 });

    // Before narrowing: i64.
    assert_eq!(
        collection_element_width(&plan, list_int),
        Some(IntWidth::I64)
    );

    narrow_collection_elements(&mut plan, &pool);

    // After narrowing: i8 — this assertion fails without the Phase C code.
    assert_eq!(
        collection_element_width(&plan, list_int),
        Some(IntWidth::I8),
        "semantic pin: narrowing must change element width from I64 to I8"
    );
}

#[test]
fn phase_c_negative_pin_wide_range_stays_canonical() {
    // Negative pin: full i64 range stays at i64.
    let mut pool = Pool::default();
    let list_int = pool.list(Idx::INT);

    let mut plan = ReprPlan::new(NarrowingPolicy::Aggressive);
    setup_list_collection(&mut plan, list_int);

    plan.join_element_range(
        list_int,
        ValueRange::Bounded {
            lo: i64::MIN,
            hi: i64::MAX,
        },
    );

    narrow_collection_elements(&mut plan, &pool);

    assert_eq!(
        collection_element_width(&plan, list_int),
        Some(IntWidth::I64),
        "negative pin: full i64 range must stay canonical"
    );
}

#[test]
fn phase_c_multiple_construction_sites_join_ranges() {
    // Two construction sites with different ranges → joined range.
    let mut pool = Pool::default();
    let list_int = pool.list(Idx::INT);

    let mut plan = ReprPlan::new(NarrowingPolicy::Aggressive);
    setup_list_collection(&mut plan, list_int);

    // Site A: [0, 50], Site B: [-100, 0] → joined: [-100, 50] → i8.
    plan.join_element_range(list_int, ValueRange::Bounded { lo: 0, hi: 50 });
    plan.join_element_range(list_int, ValueRange::Bounded { lo: -100, hi: 0 });

    narrow_collection_elements(&mut plan, &pool);

    assert_eq!(
        collection_element_width(&plan, list_int),
        Some(IntWidth::I8),
        "joined range [-100, 50] should narrow to i8"
    );
}

#[test]
fn phase_c_multiple_sites_one_wide_prevents_narrowing() {
    // One construction site with wide range prevents narrowing.
    let mut pool = Pool::default();
    let list_int = pool.list(Idx::INT);

    let mut plan = ReprPlan::new(NarrowingPolicy::Aggressive);
    setup_list_collection(&mut plan, list_int);

    // Site A: [0, 50], Site B: Top → joined: Top → no narrowing.
    plan.join_element_range(list_int, ValueRange::Bounded { lo: 0, hi: 50 });
    plan.join_element_range(list_int, ValueRange::Top);

    narrow_collection_elements(&mut plan, &pool);

    assert_eq!(
        collection_element_width(&plan, list_int),
        Some(IntWidth::I64),
        "one Top site should prevent narrowing"
    );
}

#[test]
fn phase_c_i32_boundary_narrows_correctly() {
    let mut pool = Pool::default();
    let list_int = pool.list(Idx::INT);

    let mut plan = ReprPlan::new(NarrowingPolicy::Aggressive);
    setup_list_collection(&mut plan, list_int);

    // Range fits in i32 but not i16.
    plan.join_element_range(
        list_int,
        ValueRange::Bounded {
            lo: -100_000,
            hi: 100_000,
        },
    );

    narrow_collection_elements(&mut plan, &pool);

    assert_eq!(
        collection_element_width(&plan, list_int),
        Some(IntWidth::I32),
        "range [-100000, 100000] should narrow to i32"
    );
}

/// Negative pin: `Set<int>` is excluded from Phase C narrowing because
/// eq/hash thunks always load canonical-width (i64) values from element
/// pointers. Narrowing set elements would cause the thunks to read past
/// the narrowed slot.
#[test]
fn phase_c_set_int_not_narrowed() {
    let mut pool = Pool::default();
    let set_int = pool.set(Idx::INT);

    let mut plan = ReprPlan::new(NarrowingPolicy::Aggressive);
    setup_list_collection(&mut plan, set_int); // same repr shape as list

    // Even with a tight range that would normally narrow to i8...
    plan.join_element_range(set_int, ValueRange::Bounded { lo: 0, hi: 10 });

    narrow_collection_elements(&mut plan, &pool);

    // ...Set<int> stays at canonical i64 (not narrowed).
    assert_eq!(
        collection_element_width(&plan, set_int),
        Some(IntWidth::I64),
        "Set<int> must NOT be narrowed — eq/hash thunks are not narrowing-aware"
    );
}

/// When both `[int]` and `Set<int>` have element ranges, only the list
/// is narrowed. The set stays canonical.
#[test]
fn phase_c_list_narrowed_but_set_stays_canonical() {
    let mut pool = Pool::default();
    let list_int = pool.list(Idx::INT);
    let set_int = pool.set(Idx::INT);

    let mut plan = ReprPlan::new(NarrowingPolicy::Aggressive);
    setup_list_collection(&mut plan, list_int);
    setup_list_collection(&mut plan, set_int);

    plan.join_element_range(list_int, ValueRange::Bounded { lo: 0, hi: 100 });
    plan.join_element_range(set_int, ValueRange::Bounded { lo: 0, hi: 100 });

    narrow_collection_elements(&mut plan, &pool);

    assert_eq!(
        collection_element_width(&plan, list_int),
        Some(IntWidth::I8),
        "List<int> with range [0, 100] should narrow to i8"
    );
    assert_eq!(
        collection_element_width(&plan, set_int),
        Some(IntWidth::I64),
        "Set<int> must stay at canonical i64 even with same tight range"
    );
}

/// Semantic pin: imported collection surface does NOT suppress Phase C narrowing.
/// This drives `narrow_collection_elements()` with an imported surface and
/// proves the element width actually changes to i8 (not just `!is_public_type()`).
/// Regression:
#[test]
fn phase_c_imported_surface_allows_narrowing() {
    let mut pool = Pool::default();
    let list_int = pool.list(Idx::INT);
    let list_hash = pool.hash(list_int);

    // Build a plan that simulates an imported collection surface.
    // After fix, imported surfaces don't add to pub_type_indices.
    let mut plan = crate::compute_repr_plan_with_interner(
        &pool,
        &[],
        NarrowingPolicy::Aggressive,
        &[],
        None,
        &[], // No local pub types
        &[],
        &[list_hash], // Imported collection surface
        &[],
        false,
    );

    // Seed element range and run Phase C narrowing.
    plan.join_element_range(list_int, ValueRange::Bounded { lo: -128, hi: 127 });
    narrow_collection_elements(&mut plan, &pool);

    // The imported surface should NOT block narrowing — elements narrow to i8.
    assert_eq!(
        collection_element_width(&plan, list_int),
        Some(IntWidth::I8),
        "Imported surface must not block Phase C narrowing"
    );
}

/// Negative pin: local public function DOES suppress Phase C narrowing.
/// Counterpart to `phase_c_imported_surface_allows_narrowing` — proves
/// the `is_public_type()` gate is still active for same-module public APIs.
#[test]
fn phase_c_local_public_blocks_narrowing() {
    let mut pool = Pool::default();
    let list_int = pool.list(Idx::INT);

    let mut plan = ReprPlan::new(NarrowingPolicy::Aggressive);
    setup_list_collection(&mut plan, list_int);
    plan.set_pub_type_indices([list_int]); // Local public function has [int] in signature

    plan.join_element_range(list_int, ValueRange::Bounded { lo: -128, hi: 127 });
    narrow_collection_elements(&mut plan, &pool);

    assert_eq!(
        collection_element_width(&plan, list_int),
        Some(IntWidth::I64),
        "Local public [int] must suppress narrowing (ABI safety)"
    );
}

// Phase C — update_element_summaries: Apply instruction handling

/// Regression: An Apply instruction returning [int] must widen
/// the element summary to Top, preventing unsound narrowing when push/map/
/// user functions produce elements outside the literal-only range.
#[test]
fn phase_c_apply_returning_list_int_widens_to_top() {
    use ori_arc::ir::{ArcInstr, ArgOwnership};
    use ori_arc::ArcVarId;
    use std::collections::HashMap;

    let mut pool = Pool::default();
    let list_int = pool.list(Idx::INT);
    let int_ty = Idx::INT;

    // Simulate: let dst: [int] = push(src, elem)
    let dst = ArcVarId::new(0);
    let src = ArcVarId::new(1);
    let elem = ArcVarId::new(2);
    let func_name = Name::from_raw(42);

    let instr = ArcInstr::Apply {
        dst,
        ty: list_int,
        func: func_name,
        args: vec![src, elem],
        arg_ownership: vec![ArgOwnership::Owned, ArgOwnership::Owned],
    };

    let ranges: HashMap<ArcVarId, ValueRange> = HashMap::new();
    let var_types = vec![list_int, list_int, int_ty];

    let mut table = ElementSummaryTable::new();
    update_element_summaries(&instr, &ranges, &var_types, &pool, &mut table);

    assert_eq!(
        table.element_range(list_int),
        ValueRange::Top,
        "Apply returning [int] must widen element range to Top"
    );
}

/// When both a literal construction [1] and an Apply returning [int] exist,
/// the joined element range must be Top (not [1,1]).
#[test]
fn phase_c_literal_plus_apply_widens_to_top() {
    use ori_arc::ir::{ArcInstr, ArgOwnership, CtorKind};
    use ori_arc::ArcVarId;
    use std::collections::HashMap;

    let mut pool = Pool::default();
    let list_int = pool.list(Idx::INT);
    let int_ty = Idx::INT;

    let elem_var = ArcVarId::new(0);
    let dst_literal = ArcVarId::new(1);
    let dst_apply = ArcVarId::new(2);
    let src_var = ArcVarId::new(3);

    // First: Construct(ListLiteral, [elem_var]) where elem_var range = [1, 1]
    let literal_instr = ArcInstr::Construct {
        dst: dst_literal,
        ty: list_int,
        ctor: CtorKind::ListLiteral,
        args: vec![elem_var],
    };

    // Then: Apply { func: "push", ty: [int] }
    let apply_instr = ArcInstr::Apply {
        dst: dst_apply,
        ty: list_int,
        func: Name::from_raw(42),
        args: vec![src_var, elem_var],
        arg_ownership: vec![ArgOwnership::Owned, ArgOwnership::Owned],
    };

    let mut ranges: HashMap<ArcVarId, ValueRange> = HashMap::new();
    ranges.insert(elem_var, ValueRange::Bounded { lo: 1, hi: 1 });
    let var_types = vec![int_ty, list_int, list_int, list_int];

    let mut table = ElementSummaryTable::new();

    // Process literal first — sets range to [1, 1]
    update_element_summaries(&literal_instr, &ranges, &var_types, &pool, &mut table);
    assert_eq!(
        table.element_range(list_int),
        ValueRange::Bounded { lo: 1, hi: 1 },
        "after literal only, range should be [1, 1]"
    );

    // Process Apply — must widen to Top
    update_element_summaries(&apply_instr, &ranges, &var_types, &pool, &mut table);
    assert_eq!(
        table.element_range(list_int),
        ValueRange::Top,
        "after Apply returning [int], range must be Top"
    );
}

/// Negative pin: Apply returning a non-collection type (e.g., int) must not
/// affect element summaries.
#[test]
fn phase_c_apply_returning_int_does_not_affect_elements() {
    use ori_arc::ir::{ArcInstr, ArgOwnership};
    use ori_arc::ArcVarId;
    use std::collections::HashMap;

    let pool = Pool::default();
    let int_ty = Idx::INT;

    let dst = ArcVarId::new(0);
    let arg = ArcVarId::new(1);

    let instr = ArcInstr::Apply {
        dst,
        ty: int_ty,
        func: Name::from_raw(99),
        args: vec![arg],
        arg_ownership: vec![ArgOwnership::Owned],
    };

    let ranges: HashMap<ArcVarId, ValueRange> = HashMap::new();
    let var_types = vec![int_ty, int_ty];

    let mut table = ElementSummaryTable::new();
    update_element_summaries(&instr, &ranges, &var_types, &pool, &mut table);

    // No collection type involved — table must remain truly empty.
    // We check observation_count() to distinguish "no observations" from
    // "accidentally inserted Top" (both return Top from element_range()).
    assert_eq!(
        table.observation_count(),
        0,
        "Apply returning int must not insert any element summary entries"
    );
}

/// Invoke (unwinding call) returning [int] must also widen to Top.
/// This is the most common path for push/insert — they're lowered as
/// Invoke because they can panic.
#[test]
fn phase_c_invoke_returning_list_int_widens_to_top() {
    use crate::range::field_summary::update_element_summaries_from_terminator;
    use ori_arc::ir::{ArcTerminator, ArgOwnership};
    use ori_arc::{ArcBlockId, ArcVarId};

    let mut pool = Pool::default();
    let list_int = pool.list(Idx::INT);

    let terminator = ArcTerminator::Invoke {
        dst: ArcVarId::new(0),
        ty: list_int,
        func: Name::from_raw(42),
        args: vec![ArcVarId::new(1), ArcVarId::new(2)],
        arg_ownership: vec![ArgOwnership::Owned, ArgOwnership::Owned],
        normal: ArcBlockId::new(1),
        unwind: ArcBlockId::new(2),
    };

    let mut table = ElementSummaryTable::new();
    update_element_summaries_from_terminator(&terminator, &pool, &mut table);

    assert_eq!(
        table.element_range(list_int),
        ValueRange::Top,
        "Invoke returning [int] must widen element range to Top"
    );
}

/// `ApplyIndirect` (closure call) returning `[int]` must also widen to Top.
#[test]
fn phase_c_apply_indirect_returning_list_int_widens_to_top() {
    use ori_arc::ir::ArcInstr;
    use ori_arc::ArcVarId;
    use std::collections::HashMap;

    let mut pool = Pool::default();
    let list_int = pool.list(Idx::INT);

    let dst = ArcVarId::new(0);
    let closure = ArcVarId::new(1);

    let instr = ArcInstr::ApplyIndirect {
        dst,
        ty: list_int,
        closure,
        args: vec![],
        arg_ownership: vec![],
    };

    let ranges: HashMap<ArcVarId, ValueRange> = HashMap::new();
    let var_types = vec![list_int, list_int];

    let mut table = ElementSummaryTable::new();
    update_element_summaries(&instr, &ranges, &var_types, &pool, &mut table);

    assert_eq!(
        table.element_range(list_int),
        ValueRange::Top,
        "ApplyIndirect returning [int] must widen element range to Top"
    );
}
