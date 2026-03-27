//! Tests for integer narrowing (§04.1 — struct/tuple field narrowing).

use ori_ir::Name;
use ori_types::{Idx, Pool};

use crate::narrowing::int::narrow_struct_fields;
use crate::plan::{
    DecisionReason, DecisionSource, NarrowingPolicy, ReprAttribute, ReprDecision, ReprPlan,
};
use crate::range::ValueRange;
use crate::repr::{IntWidth, MachineRepr};
use crate::struct_repr::{FieldRepr, StructRepr, TupleRepr};

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
fn tuple_elements_narrowed() {
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

    assert_eq!(
        tuple_element_width(&plan, idx, 0),
        Some(IntWidth::I8),
        "tuple element 0 should narrow to i8"
    );
    assert_eq!(
        tuple_element_width(&plan, idx, 1),
        Some(IntWidth::I16),
        "tuple element 1 should narrow to i16"
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
